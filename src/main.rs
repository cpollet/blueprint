mod domain;
mod parser;
mod ppm;
mod ui;

use crate::domain::{Blueprint, Bound, Color, Draw, Edge, Point, Shape};
use crate::parser::Coord;
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

    let canvas = Canvas::try_from(blueprint)
        .expect("Failed to convert blueprint")
        .pad(50, 50);

    PpmImage::from(&canvas)
        .write_to_file(&out_filename)
        .unwrap();

    ui::show(PathBuf::from(in_filename), Blueprint::<usize>::default()).expect("can launch UI");
}

fn load_blueprint(path: &Path) -> Result<Blueprint<usize>, ()> {
    let src = fs::read_to_string(path).expect("Failed to read file");
    let shapes = parser::parse(src.as_str(), path);

    let mut blueprint = Blueprint::default();
    let mut points = HashMap::new();

    for edge_starts in shapes {
        if edge_starts.is_empty() {
            continue;
        }

        let mut nodes: Vec<Point<i32>> = Vec::with_capacity(edge_starts.len());
        let mut edges = Vec::with_capacity(edge_starts.len() - 1);

        for ((from, attr), to) in edge_starts
            .iter()
            .map(|i| (&i.coord, &i.attributes))
            .zip(edge_starts.iter().skip(1).map(|i| &i.coord))
        {
            let (from, tag) = match from {
                Coord::Absolute(x, y, tag) => (Point::new(*x, *y), *tag),
                Coord::Relative(x, y, tag) => (
                    nodes
                        .last()
                        .copied()
                        .map(|last| last.add(*x, *y))
                        .unwrap_or(Point::new(*y, *x)),
                    *tag,
                ),
                Coord::Reference(tag) => (
                    *points.get(*tag).unwrap_or_else(|| {
                        eprintln!("#{tag} not found",);
                        exit(1);
                    }),
                    None,
                ),
            };
            nodes.push(from);
            if let Some(tag) = tag {
                // println!("{} at {:?}", tag, from);
                points.insert(tag, from);
            }

            let to = match to {
                Coord::Absolute(x, y, _) => Point::new(*x, *y),
                Coord::Relative(x, y, _) => nodes
                    .last()
                    .copied()
                    .map(|last: Point<i32>| last.add(*x, *y))
                    .unwrap_or(Point::new(*y, *x)),

                Coord::Reference(tag) => *points.get(tag).unwrap_or_else(|| {
                    eprintln!("#{tag} not found",);
                    exit(1);
                }),
            };

            let color = attr
                .get("color")
                .map(|s| Color::try_from(*s))
                .map(|c| c.unwrap_or_default())
                .unwrap_or_default();

            edges.push(Edge::new_from_points(from, to, color));
        }

        blueprint.push(Shape::from(edges))
    }

    blueprint.translate_to_origin();
    Blueprint::<usize>::try_from(blueprint.clone())
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
    if matches!(
        &event.kind,
        notify::EventKind::Modify(notify::event::ModifyKind::Data(_))
    ) {
        let blueprint = load_blueprint(&event.paths[0]).unwrap();
        return Some(AppEvent::BlueprintUpdated(blueprint));
    }

    None
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

impl TryFrom<Blueprint<usize>> for Canvas {
    type Error = ();

    fn try_from(blueprint: Blueprint<usize>) -> Result<Self, Self::Error> {
        let boundaries = blueprint.boundaries();
        let (width, height) = (boundaries.1.x, boundaries.1.y);
        let mut canvas = Canvas::new(width + 1, height + 1);
        blueprint.draw(&mut canvas);

        Ok(canvas)
    }
}

impl TryFrom<Blueprint<i32>> for Canvas {
    type Error = ();

    fn try_from(mut value: Blueprint<i32>) -> Result<Self, Self::Error> {
        value.translate_to_origin();
        let blueprint = Blueprint::<usize>::try_from(value)?;
        Self::try_from(blueprint)
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
