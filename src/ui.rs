use iced::keyboard::key::Named;
use iced::mouse::Cursor;
use iced::widget::canvas::{Geometry, Path, Stroke};
use iced::widget::{MouseArea, canvas, column, container, row, text};
use iced::{
    Background, Color, Element, Event, Font, Length, Point, Rectangle, Renderer, Subscription,
    Task, Theme, Vector, border, event, keyboard, padding,
};
use std::fmt::{Display, Formatter};

pub fn show(blueprint: crate::Blueprint<usize>) -> iced::Result {
    iced::application(Blueprint::title, Blueprint::update, Blueprint::view)
        .subscription(Blueprint::subscription)
        .theme(|_| Theme::Light)
        .default_font(Font::MONOSPACE)
        .run_with(|| (Blueprint::new(blueprint), Task::none()))
}

#[derive(Debug)]
struct Blueprint {
    zoom_level: ZoomLevel,
    translation: Vector,
    mouse_position: Point,
    fixed_position: Option<Point>,
    blueprint: DrawableBlueprint,
    raw_blueprint: crate::Blueprint<usize>,
}

impl Blueprint {
    fn new(blueprint: crate::Blueprint<usize>) -> Self {
        let translation = Vector::new(50.0, 50.0);
        Self {
            zoom_level: ZoomLevel::default(),
            translation,
            mouse_position: Default::default(),
            fixed_position: None,
            blueprint: DrawableBlueprint {
                blueprint: blueprint.clone(),
                translation,
            },
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
            Message::TranslateUp => self.translation.y -= 10.0,
            Message::TranslateLeft => self.translation.x -= 10.0,
            Message::TranslateDown => self.translation.y += 10.0,
            Message::TranslateRight => self.translation.x += 10.0,
            Message::CursorMoved(point) => self.mouse_position = point,
            Message::StorePosition => {
                self.fixed_position = Some(self.mouse_position);
            }
            Message::DropPosition => {
                self.fixed_position = None;
            }
        }

        self.blueprint = DrawableBlueprint {
            blueprint: self.raw_blueprint.scale(self.zoom_level.scale_factor()),
            translation: self.translation,
        };
    }

    fn subscription(&self) -> Subscription<Message> {
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
            _ => None,
        })
    }

    fn view(&self) -> Element<'_, Message> {
        let zoom_level = text(format!("zoom: {}", self.zoom_level));
        let mouse_position = text(format!(
            "mouse: {}, {}",
            self.mouse_position.x.floor(),
            self.mouse_position.y.floor()
        ));
        let delta = self.fixed_position.map(|position| {
            let dx = ((self.mouse_position.x - position.x) / self.zoom_level.scale_factor())
                .floor()
                .abs();
            let dy = ((self.mouse_position.y - position.y) / self.zoom_level.scale_factor())
                .floor()
                .abs();
            text(format!("dx: {dx}, dy: {dy}; area: {}", dx * dy))
        });

        let header = row![zoom_level, mouse_position]
            .push_maybe(delta)
            .spacing(20);

        let image = canvas(&self.blueprint)
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
            container(image)
                .style(|_| container::Style::default()
                    .border(border::width(1).color(Color::from(crate::Color::Cyan)))
                    .background(Background::Color(Color::from(crate::Color::White))))
                // .style(|_| container::Style::default()
                //     .border(border::width(1).color(Color::from(crate::Color::Cyan)))
                //     .background(Background::Color(Color::from(crate::Color::Magenta))))
                .width(Length::Fill)
                .height(Length::Fill),
        ]
        .width(Length::Fill)
        .height(Length::Fill);

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
enum Message {
    ZoomIn,
    ZoomOut,
    ZoomReset,
    CursorMoved(Point),
    StorePosition,
    DropPosition,
    TranslateUp,
    TranslateLeft,
    TranslateDown,
    TranslateRight,
}

#[derive(Debug)]
struct DrawableBlueprint {
    blueprint: crate::Blueprint<usize>,
    translation: Vector,
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

                let stroke = Stroke::default().with_color(edge.color().into());

                frame.stroke(&line, stroke);
            }
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
