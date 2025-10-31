use crate::open_and_watch_file;
use futures::channel::mpsc::Sender;
use iced::alignment::{Horizontal, Vertical};
use iced::keyboard::key::Named;
use iced::mouse::Cursor;
use iced::widget::canvas::{Geometry, Path, Stroke, Text};
use iced::widget::{MouseArea, canvas, column, container, row, text};
use iced::{
    Color, Element, Event, Font, Length, Point, Rectangle, Renderer, Subscription, Task, Theme,
    Vector, border, event, keyboard, padding,
};
use std::fmt::{Debug, Display, Formatter};
use std::ops::{Add, Sub};
use std::path::PathBuf;

pub fn show(path: PathBuf, blueprint: crate::Blueprint<usize>) -> iced::Result {
    iced::application(Blueprint::title, Blueprint::update, Blueprint::view)
        .subscription(Blueprint::subscription)
        .theme(|_| Theme::Light)
        .default_font(Font::MONOSPACE)
        .run_with(|| (Blueprint::new(path, blueprint), Task::none()))
}

/// events received by the UI
pub enum AppEvent {
    Ready(Sender<Command>),
    BlueprintUpdated(crate::Blueprint<usize>),
}

/// commands sent from the UI
#[derive(Debug)]
pub enum Command {
    OpenFile(PathBuf),
}

#[derive(Debug)]
struct Blueprint {
    path: PathBuf,
    sender: Option<Sender<Command>>,
    zoom_level: ZoomLevel,
    translation: Vector,
    fixed_translation: Option<Vector>,
    mouse_position: Point,
    mouse_mode: MouseMode,
    fixed_position: Option<Point>,
    raw_blueprint: crate::Blueprint<usize>,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum MouseMode {
    #[default]
    Select,
    Move,
}

impl Blueprint {
    fn new(path: PathBuf, blueprint: crate::Blueprint<usize>) -> Self {
        let translation = Vector::new(50.0, 50.0);
        Self {
            path,
            sender: None,
            zoom_level: ZoomLevel::default(),
            translation,
            fixed_translation: None,
            mouse_position: Default::default(),
            mouse_mode: Default::default(),
            fixed_position: None,
            raw_blueprint: blueprint,
        }
    }
}

impl Blueprint {
    fn update(&mut self, message: Message) {
        match message {
            Message::ZoomIn => {
                self.zoom_level = self.zoom_level.zoom_in();
            }
            Message::ZoomOut => {
                self.zoom_level = self.zoom_level.zoom_out();
            }
            Message::ZoomReset => {
                self.zoom_level = ZoomLevel::default();
                self.translation = Vector::new(50.0, 50.0);
            }
            Message::TranslateUp => self.translation.y -= 1.0,
            Message::TranslateLeft => self.translation.x -= 1.0,
            Message::TranslateDown => self.translation.y += 1.0,
            Message::TranslateRight => self.translation.x += 1.0,
            Message::CursorMoved(point) => {
                self.mouse_position = point;

                if matches!(self.mouse_mode, MouseMode::Move)
                    && let Some(fixed_translation) = self.fixed_translation
                {
                    self.translation = fixed_translation.add(Vector::new(
                        self.mouse_position.x
                            - self.fixed_position.unwrap_or(self.mouse_position).x,
                        self.mouse_position.y
                            - self.fixed_position.unwrap_or(self.mouse_position).y,
                    ));
                }
            }
            Message::ChangeMouseMode(mode) => {
                self.mouse_mode = mode;
            }
            Message::StorePosition => {
                self.fixed_translation = Some(self.translation);
                self.fixed_position = Some(self.mouse_position);
            }
            Message::DropPosition => {
                self.fixed_translation = None;
                self.fixed_position = None;
            }
            Message::BlueprintUpdated(blueprint) => {
                println!("Blueprint reloaded");
                self.raw_blueprint = blueprint;
            }
            Message::SetSender(sender) => {
                self.sender = Some(sender);
                self.sender
                    .as_mut()
                    .unwrap()
                    .try_send(Command::OpenFile(self.path.clone()))
                    .unwrap();
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(vec![
            Subscription::run(open_and_watch_file).map(|e| match e {
                AppEvent::BlueprintUpdated(blueprint) => Message::BlueprintUpdated(blueprint),
                AppEvent::Ready(sender) => Message::SetSender(sender),
            }),
            event::listen_with(|e, _, _| match e {
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Character(c),
                    modifiers,
                    ..
                }) if modifiers.is_empty() => match c.as_str() {
                    "i" | "e" => Some(Message::ZoomIn),
                    "o" | "q" => Some(Message::ZoomOut),
                    "w" => Some(Message::TranslateUp),
                    "a" => Some(Message::TranslateLeft),
                    "s" => Some(Message::TranslateDown),
                    "d" => Some(Message::TranslateRight),
                    "0" => Some(Message::ZoomReset),
                    _ => None,
                },
                Event::Keyboard(keyboard::Event::KeyReleased {
                    key: keyboard::Key::Named(Named::Space),
                    modifiers,
                    ..
                }) if modifiers.is_empty() => Some(Message::StorePosition),
                Event::Keyboard(keyboard::Event::KeyReleased {
                    key: keyboard::Key::Named(Named::Escape),
                    modifiers,
                    ..
                }) if modifiers.is_empty() => Some(Message::DropPosition),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(Named::Control),
                    ..
                }) => Some(Message::ChangeMouseMode(MouseMode::Move)),
                Event::Keyboard(keyboard::Event::KeyReleased {
                    key: keyboard::Key::Named(Named::Control),
                    ..
                }) => Some(Message::ChangeMouseMode(Default::default())),
                _ => None,
            }),
        ])
    }

    fn view(&self) -> Element<'_, Message> {
        let zoom_level = text(format!("zoom: {}", self.zoom_level));
        let mouse_position = text(format!(
            "mouse: {}, {}",
            self.mouse_position.x.floor(),
            self.mouse_position.y.floor()
        ));

        let distances = self
            .fixed_position
            .filter(|_| matches!(self.mouse_mode, MouseMode::Select))
            .map(|position| Distances::from(self.mouse_position, position, self.zoom_level));

        let delta = distances.map(|d| {
            text(format!(
                "dx: {}, dy: {}; area: {}",
                d.horizontal.floor(),
                d.vertical.floor(),
                d.diagonal.floor()
            ))
        });

        let header = row![zoom_level, mouse_position]
            .push_maybe(delta)
            .spacing(20);

        let image = canvas(DrawableBlueprint {
            blueprint: self.raw_blueprint.scale(self.zoom_level.scale_factor()),
            translation: self.translation,
            zoom_level: self.zoom_level,
            mouse_position: self.mouse_position,
            distances: self.fixed_position.zip(distances),
        })
        .width(Length::Fill)
        .height(Length::Fill);

        let image = MouseArea::new(image)
            .on_move(Message::CursorMoved)
            .on_release(Message::DropPosition)
            .on_press(Message::StorePosition);

        let rows = column![
            container(header)
                .style(|_| container::Style::default()
                    .border(border::width(1).color(Color::from(crate::Color::Cyan))))
                .padding(padding::bottom(5).top(5)),
            container(image).style(|_| container::Style::default()
                // .background(Background::Color(Color::from(crate::Color::Magenta)))
                .border(border::width(1).color(Color::from(crate::Color::Cyan))))
        ];

        container(rows)
            .padding(10)
            .width(Length::Fill)
            .height(Length::Fill)
            // .style(|_| {
            //     container::Style::default()
            //         .background(Background::Color(Color::from(crate::Color::Yellow)))
            // })
            .into()
    }

    fn title(&self) -> String {
        "Blueprint".into()
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    ZoomIn,
    ZoomOut,
    ZoomReset,
    CursorMoved(Point),
    ChangeMouseMode(MouseMode),
    StorePosition,
    DropPosition,
    TranslateUp,
    TranslateLeft,
    TranslateDown,
    TranslateRight,
    BlueprintUpdated(crate::Blueprint<usize>),
    SetSender(Sender<Command>),
}

#[derive(Debug)]
struct DrawableBlueprint {
    blueprint: crate::Blueprint<usize>,
    translation: Vector,
    zoom_level: ZoomLevel,
    mouse_position: Point,
    distances: Option<(Point, Distances)>,
}

impl<Message> canvas::Program<Message> for DrawableBlueprint {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        frame.translate(self.translation);

        for shape in self.blueprint.shapes_iter() {
            for edge in shape.edges_iter() {
                if edge.color().is_transparent() {
                    continue;
                }

                let line = Path::line(edge.from.into(), edge.to.into());

                frame.stroke(&line, Stroke::default().with_color(edge.color().into()));
            }
        }

        if let Some((fixed_position, distances)) = self.distances {
            let top_left = fixed_position.sub(self.translation);
            let bottom_right = self.mouse_position.sub(self.translation);
            let top_right = Point::new(bottom_right.x, top_left.y);
            let bottom_left = Point::new(top_left.x, bottom_right.y);

            let lhline = Path::line(top_left, top_right);
            frame.stroke(
                &lhline,
                Stroke::default().with_color(Color::new(1., 0., 1., 1.0)),
            );
            let rhline = Path::line(bottom_left, bottom_right);
            frame.stroke(
                &rhline,
                Stroke::default().with_color(Color::new(0.8, 0.8, 0.8, 0.8)),
            );

            let vtline = Path::line(top_left, bottom_left);
            frame.stroke(
                &vtline,
                Stroke::default().with_color(Color::new(1., 0., 1., 1.0)),
            );
            let vbline = Path::line(top_right, bottom_right);
            frame.stroke(
                &vbline,
                Stroke::default().with_color(Color::new(0.8, 0.8, 0.8, 1.0)),
            );

            let dline = Path::line(top_left, bottom_right);
            frame.stroke(
                &dline,
                Stroke::default().with_color(Color::new(1., 0., 1., 1.0)),
            );

            let mut hdistance = Text::from(format!("{}", distances.horizontal.floor()));
            hdistance.horizontal_alignment = Horizontal::Center;
            hdistance.vertical_alignment = Vertical::Center;
            hdistance.position = Point::new((top_left.x + top_right.x) / 2., top_left.y - 10.);
            frame.fill_text(hdistance);

            let mut vdistance = Text::from(format!("{}", distances.vertical.floor()));
            vdistance.position = Point::new(top_left.x + 15., (top_left.y + bottom_left.y) / 2.);
            vdistance.horizontal_alignment = Horizontal::Center;
            vdistance.vertical_alignment = Vertical::Center;
            frame.fill_text(vdistance);

            let mut ddistance = Text::from(format!("{}", distances.diagonal.floor()));
            ddistance.horizontal_alignment = Horizontal::Center;
            ddistance.vertical_alignment = Vertical::Center;
            ddistance.position = Point::new(
                top_left.x + distances.horizontal * self.zoom_level.scale_factor() * 0.75,
                top_left.y + distances.vertical * self.zoom_level.scale_factor() * 0.75,
            );
            frame.fill_text(ddistance);
        }
        vec![frame.into_geometry()]
    }
}

impl From<crate::Point<usize>> for Point {
    fn from(value: crate::domain::Point<usize>) -> Self {
        Self {
            x: value.x as f32,
            y: value.y as f32,
        }
    }
}

impl From<crate::Color> for Color {
    fn from(value: crate::Color) -> Self {
        let (r, g, b, a) = value.as_rgba();
        Self {
            r: r as f32 / 255.,
            g: g as f32 / 255.,
            b: b as f32 / 255.,
            a: a as f32 / 255.,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct ZoomLevel {
    num: u8,
    denum: u8,
}

impl ZoomLevel {
    fn zoom_in(self) -> Self {
        if self.denum == 1 {
            Self {
                num: self.num + 1,
                denum: self.denum,
            }
        } else {
            Self {
                num: self.num,
                denum: self.denum - 1,
            }
        }
    }

    fn zoom_out(self) -> Self {
        if self.num == 1 {
            Self {
                num: self.num,
                denum: self.denum + 1,
            }
        } else {
            Self {
                num: self.num - 1,
                denum: self.denum,
            }
        }
    }

    fn scale_factor(&self) -> f32 {
        self.num as f32 / self.denum as f32
    }
}

impl Display for ZoomLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.denum > 1 {
            write!(f, "{}/{}", self.num, self.denum)
        } else {
            write!(f, "{}", self.num)
        }
    }
}

impl Default for ZoomLevel {
    fn default() -> Self {
        Self { num: 1, denum: 1 }
    }
}

#[derive(Debug, Copy, Clone)]
struct Distances {
    horizontal: f32,
    vertical: f32,
    diagonal: f32,
}

impl Distances {
    fn from(p1: Point, p2: Point, zoom_level: ZoomLevel) -> Self {
        Self {
            horizontal: ((p1.x - p2.x) / zoom_level.scale_factor()).abs(),
            vertical: ((p1.y - p2.y) / zoom_level.scale_factor()).abs(),
            diagonal: (p1.distance(p2)) / zoom_level.scale_factor(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ui::ZoomLevel;
    use iced::Color;

    #[test]
    fn test_color() {
        let color = Color::from(crate::Color::Red);
        assert_eq!(color, Color::from_rgba(1., 0., 0., 1.));
    }

    #[test]
    fn zoom() {
        let zoom = ZoomLevel::default();
        assert_eq!(zoom, ZoomLevel { num: 1, denum: 1 });
        let zoom = zoom.zoom_in();
        assert_eq!(zoom, ZoomLevel { num: 2, denum: 1 });
        let zoom = zoom.zoom_in();
        assert_eq!(zoom, ZoomLevel { num: 3, denum: 1 });
        let zoom = zoom.zoom_out();
        assert_eq!(zoom, ZoomLevel { num: 2, denum: 1 });
        let zoom = zoom.zoom_out();
        assert_eq!(zoom, ZoomLevel { num: 1, denum: 1 });
        let zoom = zoom.zoom_out();
        assert_eq!(zoom, ZoomLevel { num: 1, denum: 2 });
        let zoom = zoom.zoom_out();
        assert_eq!(zoom, ZoomLevel { num: 1, denum: 3 });
        let zoom = zoom.zoom_in();
        assert_eq!(zoom, ZoomLevel { num: 1, denum: 2 });
        let zoom = zoom.zoom_in();
        assert_eq!(zoom, ZoomLevel { num: 1, denum: 1 });
        let zoom = zoom.zoom_in();
        assert_eq!(zoom, ZoomLevel { num: 2, denum: 1 });
    }
}
