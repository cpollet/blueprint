use iced::keyboard::key::Named;
use iced::mouse::Cursor;
use iced::widget::canvas::{Geometry, Path, Stroke};
use iced::widget::image::Handle;
use iced::widget::{MouseArea, canvas, column, container, row, text};
use iced::{
    Background, Color, Element, Event, Font, Length, Point, Rectangle, Renderer, Subscription,
    Task, Theme, Vector, border, event, keyboard, padding,
};

pub fn show(blueprint: crate::Blueprint<usize>) -> iced::Result {
    iced::application(Blueprint::title, Blueprint::update, Blueprint::view)
        .subscription(Blueprint::subscription)
        .theme(|_| Theme::Light)
        .default_font(Font::MONOSPACE)
        .run_with(|| (Blueprint::new(blueprint), Task::none()))
}

#[derive(Debug)]
struct Blueprint {
    zoom_level: i32,
    mouse: Point,
    position: Option<Point>,
    _image: Handle,
    blueprint: crate::Blueprint<usize>,
}

impl Blueprint {
    fn new(blueprint: crate::Blueprint<usize>) -> Self {
        Self {
            zoom_level: 0,
            mouse: Default::default(),
            position: None,
            _image: Handle::from_bytes(include_str!("../local/home.ppm")),
            blueprint,
        }
    }
}

impl Blueprint {
    fn update(&mut self, message: Message) {
        match message {
            Message::ZoomIn => self.zoom_level += 1,
            Message::ZoomOut => self.zoom_level -= 1,
            Message::ZoomReset => self.zoom_level = 0,
            Message::CursorMoved(point) => self.mouse = point,
            Message::StorePosition => {
                self.position = Some(self.mouse);
            }
            Message::DropPosition => {
                self.position = None;
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        event::listen_with(|e, _, _| match e {
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Character(c),
                modifiers,
                ..
            }) if modifiers.is_empty() => match c.as_str() {
                "i" => Some(Message::ZoomIn),
                "o" => Some(Message::ZoomOut),
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
        let zoom_level = text(format!("zoom level: {}", self.zoom_level));
        let mouse_position = text(format!(
            "mouse: {}, {}",
            self.mouse.x.floor(),
            self.mouse.y.floor()
        ));
        let delta = self.position.map(|position| {
            text(format!(
                "dx: {}, dy: {}; area: {}",
                (self.mouse.x - position.x).floor(),
                (self.mouse.y - position.y).floor(),
                ((self.mouse.x - position.x) * (self.mouse.y - position.y)).floor()
            ))
        });

        let header = row![zoom_level, mouse_position]
            .push_maybe(delta)
            .spacing(20);

        // let image = image(self.image.clone()).content_fit(ContentFit::None);

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
}

impl<Message> canvas::Program<Message> for crate::Blueprint<usize> {
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
        frame.translate(Vector::new(50., 50.));
        for shape in self.shapes_iter() {
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

#[cfg(test)]
mod tests {
    use iced::Color;

    #[test]
    fn test_color() {
        let color = Color::from(crate::Color::Red);
        assert_eq!(color, Color::from_rgba(1., 0., 0., 1.));
    }
}
