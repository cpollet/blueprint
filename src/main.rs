mod domain;
mod lexer;
mod parser;
mod ppm;
mod ui;

use crate::domain::{Blueprint, Bound, Color, Draw, Edge, Point, Shape};
use crate::parser::{CommandKind, Coord};
use crate::ppm::PpmImage;
use crate::ui::{AppEvent, Command};
use futures::SinkExt;
use futures::Stream;
use futures::channel::mpsc;
use futures::channel::mpsc::Receiver;
use futures::{StreamExt, select};
use iced_futures::stream;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::{env, fs};

fn main() {
    let args: Vec<String> = env::args().collect();
    let in_filename = args.get(1).unwrap_or_else(|| {
        eprintln!("Usage: {} <filename>", args[0]);
        exit(1);
    });
    let out_filename = format!(
        "{}.ppm",
        in_filename
            .rsplit_once(".")
            .unwrap_or_else(|| {
                eprintln!("<filename> must end with .bp");
                exit(1)
            })
            .0
    );

    let blueprint = load_blueprint(Path::new(in_filename)).unwrap();

    let canvas = Canvas::from(blueprint).pad(50, 50);

    PpmImage::from(&canvas)
        .write_to_file(&out_filename)
        .unwrap();

    ui::show(PathBuf::from(in_filename), Blueprint::default()).expect("can launch UI");
}

struct BlueprintLoader<'s> {
    points: HashMap<&'s str, Point>,
    last_point: Option<Point>,
    stack: Vec<Point>,
    blueprint: Blueprint,
}

impl<'s> BlueprintLoader<'s> {
    pub fn new() -> Self {
        Self {
            last_point: Default::default(),
            points: Default::default(),
            stack: Default::default(),
            blueprint: Default::default(),
        }
    }

    pub fn exec(
        mut self,
        commands: &'s [parser::Command],
        lines: &[usize],
    ) -> Result<Blueprint, ()> {
        // self.nodes.reserve(commands.len());

        self.exec_block(commands, lines)?;

        self.blueprint.translate_to_origin();
        Ok(self.blueprint)
    }

    fn exec_block(
        &mut self,
        commands: &'s [parser::Command],
        newline_offsets: &[usize],
    ) -> Result<(), ()> {
        if commands.is_empty() {
            return Ok(());
        }
        let mut edges = Vec::with_capacity(commands.len() - 1);

        for command in commands {
            let (draw, to, tag) = match &command.kind {
                CommandKind::Move(Coord::Absolute(x, y, tag)) => {
                    let to = Point::new(*x as f32, *y as f32);
                    (None, to, *tag)
                }
                CommandKind::Move(Coord::Relative(dx, dy, tag)) => {
                    let from = self.last_point.unwrap_or_default();
                    let to = from.add(*dx as f32, *dy as f32);
                    (None, to, *tag)
                }
                CommandKind::Move(Coord::Reference(tag)) => {
                    let to = match self.points.get(*tag) {
                        None => {
                            eprintln!("#{tag} not found",);
                            return Err(());
                        }
                        Some(p) => *p,
                    };
                    (None, to, None)
                }
                CommandKind::Draw(Coord::Absolute(x, y, tag), color) => {
                    let from = self.last_point.unwrap_or_default();
                    let to = Point::new(*x as f32, *y as f32);
                    (Some((from, color)), to, *tag)
                }
                CommandKind::Draw(Coord::Relative(dx, dy, tag), color) => {
                    let from = self.last_point.unwrap_or_default();
                    let to = from.add(*dx as f32, *dy as f32);
                    (Some((from, color)), to, *tag)
                }
                CommandKind::Draw(Coord::Reference(tag), color) => {
                    let from = self.last_point.unwrap_or_default();
                    let to = match self.points.get(tag) {
                        None => {
                            eprintln!("#{tag} not found",);
                            return Err(());
                        }
                        Some(p) => *p,
                    };
                    (Some((from, color)), to, None)
                }
                CommandKind::Nested(commands) => {
                    if let Some(last_point) = self.last_point {
                        self.stack.push(last_point)
                    }

                    self.exec_block(commands, newline_offsets)?;

                    if let Some(last_point) = self.stack.pop() {
                        self.last_point.replace(last_point);
                    }

                    continue;
                }
            };

            if let Some((from, color)) = draw {
                let line = newline_offsets
                    .iter()
                    .enumerate()
                    .filter_map(|(i, offset)| {
                        if *offset > command.src_index {
                            Some(i)
                        } else {
                            None
                        }
                    })
                    .next()
                    .unwrap_or_default()
                    + 1;

                let edge = Edge::new_from_points(from, to, *color, line);
                edges.push(edge);
            }

            if let Some(tag) = tag {
                self.points.insert(tag, to);
            }

            self.last_point.replace(to);
        }

        self.blueprint.push(Shape::from(edges));

        Ok(())
    }
}

// todo return a String as error and display it on the UI
fn load_blueprint(path: &Path) -> Result<Blueprint, ()> {
    let src = fs::read_to_string(path).expect("Failed to read file");

    let newline_offsets = src
        .chars()
        .enumerate()
        .filter_map(|(i, c)| if c == '\n' { Some(i) } else { None })
        .collect::<Vec<usize>>();

    let commands = parser::parse(src.as_str(), path);

    BlueprintLoader::new().exec(&commands, &newline_offsets)
}

pub fn open_and_watch_file() -> impl Stream<Item = AppEvent> {
    // https://docs.rs/iced/latest/iced/struct.Subscription.html
    // https://github.com/notify-rs/notify/blob/main/examples/async_monitor.rs
    stream::channel(100, |mut output| async move {
        let (watcher, mut fs_events_rx) = async_watcher().unwrap();
        let mut watcher = FileWatcher::from(watcher);

        let (ui_commands_tx, mut ui_commands_rx) = mpsc::channel(100);
        output.send(AppEvent::Ready(ui_commands_tx)).await.unwrap();

        loop {
            let mut next_ui_command = ui_commands_rx.next();
            let mut next_fs_event = fs_events_rx.next();
            select! {
                fs_event = next_fs_event => {
                    if let Some(Ok(fs_event)) = fs_event
                        && let Some(event) = handle_fs_event(fs_event) {
                            output.send(event).await.unwrap();
                    }
                },
                ui_command = next_ui_command => {
                    if let Some(ui_command) = ui_command
                        && let Some(event) = handle_ui_command(ui_command, &mut watcher) {
                            output.send(event).await.unwrap();
                    }
                },
            }
        }
    })
}

fn async_watcher() -> notify::Result<(RecommendedWatcher, Receiver<notify::Result<notify::Event>>)>
{
    // https://github.com/notify-rs/notify/blob/main/examples/async_monitor.rs
    let (mut tx, rx) = mpsc::channel(1);

    let watcher = RecommendedWatcher::new(
        move |res| {
            futures::executor::block_on(async {
                tx.send(res).await.unwrap();
            })
        },
        Config::default(),
    )?;

    Ok((watcher, rx))
}

fn handle_fs_event(event: notify::Event) -> Option<AppEvent> {
    match &event.kind {
        notify::EventKind::Modify(notify::event::ModifyKind::Data(_))
        | notify::EventKind::Access(notify::event::AccessKind::Close(
            notify::event::AccessMode::Write,
        )) => load_blueprint(&event.paths[0])
            .ok()
            .map(AppEvent::BlueprintUpdated),
        _ => None,
    }
}

fn handle_ui_command(cmd: Command, watcher: &mut FileWatcher) -> Option<AppEvent> {
    match cmd {
        Command::OpenFile(path) => {
            let blueprint = load_blueprint(&path).unwrap();
            watcher.watch(path);
            Some(AppEvent::BlueprintUpdated(blueprint))
        }
    }
}

struct FileWatcher {
    inner: RecommendedWatcher,
    path: Option<PathBuf>,
}

impl FileWatcher {
    fn watch(&mut self, path: PathBuf) {
        if let Some(path) = self.path.take() {
            self.inner.unwatch(&path).unwrap();
        }
        self.inner
            .watch(&path, RecursiveMode::NonRecursive)
            .unwrap();
        self.path = Some(path);
    }
}

impl From<RecommendedWatcher> for FileWatcher {
    fn from(inner: RecommendedWatcher) -> Self {
        Self { inner, path: None }
    }
}

struct Canvas {
    width: usize,
    height: usize,
    pixels: Vec<Color>,
}

impl From<Blueprint> for Canvas {
    fn from(blueprint: Blueprint) -> Self {
        let boundaries = blueprint.boundaries();
        let (width, height) = (boundaries.1.x, boundaries.1.y);
        let mut canvas = Canvas::new((width + 1.).ceil() as usize, (height + 1.).ceil() as usize);
        blueprint.draw(&mut canvas);

        canvas
    }
}

impl Canvas {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![Color::White; width * height],
        }
    }

    fn set(&mut self, x: usize, y: usize, color: Color) {
        debug_assert!(x < self.width, "set width: {} >= {}", x, self.width);
        debug_assert!(y < self.height, "set height: {} >= {}", y, self.height);

        self.pixels[x + y * self.width] = color;
    }

    fn get(&self, x: usize, y: usize) -> Color {
        debug_assert!(x < self.width, "get width: {} >= {}", x, self.width);
        debug_assert!(y < self.height, "get height: {} >= {}", y, self.height);
        self.pixels[x + y * self.width]
    }

    fn pad(&self, horizontal: usize, vertical: usize) -> Self {
        let mut canvas = Canvas::new(self.width + 2 * horizontal, self.height + 2 * vertical);

        for y in 0..self.height {
            for x in 0..self.width {
                canvas.set(x + horizontal, y + vertical, self.get(x, y));
            }
        }

        canvas
    }
}
