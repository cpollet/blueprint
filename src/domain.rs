use crate::Canvas;
use std::slice::Iter;

pub trait Bound {
    fn boundaries(self) -> (Point, Point);
}

pub trait Translate {
    fn translate(&mut self, dx: f32, dy: f32);
}

pub trait Draw {
    fn draw(&self, canvas: &mut Canvas);
}

impl<I, E> Bound for I
where
    I: Iterator<Item = E>,
    E: Bound,
{
    fn boundaries(self) -> (Point, Point) {
        self.fold(
            (Point::MAX, Point::MIN),
            |(top_left, bottom_right), inner| {
                let (shape_top_left, shape_bottom_right) = inner.boundaries();
                (
                    top_left.top_left(&shape_top_left),
                    bottom_right.bottom_right(&shape_bottom_right),
                )
            },
        )
    }
}

#[derive(Default, Debug, Clone)]
pub struct Blueprint {
    shapes: Vec<Shape>,
}

impl Blueprint {
    pub fn push(&mut self, shape: Shape) {
        self.shapes.push(shape);
    }

    pub fn shapes_iter(&self) -> Iter<'_, Shape> {
        self.shapes.iter()
    }

    pub fn translate_to_origin(&mut self) {
        let boundaries = self.boundaries();
        self.translate(-boundaries.0.x, -boundaries.0.y);
    }

    pub fn scale(&self, factor: f32) -> Blueprint {
        Self {
            shapes: self
                .shapes
                .iter()
                .map(|shape| shape.scale(factor))
                .collect(),
        }
    }
}

impl Bound for &Blueprint {
    fn boundaries(self) -> (Point, Point) {
        self.shapes.iter().boundaries()
    }
}

impl Translate for Blueprint {
    fn translate(&mut self, dx: f32, dy: f32) {
        self.shapes
            .iter_mut()
            .for_each(|shape| shape.translate(dx, dy));
    }
}

impl Draw for Blueprint {
    fn draw(&self, canvas: &mut Canvas) {
        self.shapes.iter().for_each(|shape| shape.draw(canvas));
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct Shape {
    edges: Vec<Edge>,
}

impl Shape {
    pub fn edges_iter(&self) -> Iter<'_, Edge> {
        self.edges.iter()
    }

    pub fn scale(&self, factor: f32) -> Shape {
        Self {
            edges: self.edges.iter().map(|edge| edge.scale(factor)).collect(),
        }
    }
}

impl Bound for &Shape {
    fn boundaries(self) -> (Point, Point) {
        self.edges.iter().boundaries()
    }
}

impl Translate for Shape {
    fn translate(&mut self, dx: f32, dy: f32) {
        self.edges
            .iter_mut()
            .for_each(|edge| edge.translate(dx, dy));
    }
}

impl Draw for Shape {
    fn draw(&self, canvas: &mut Canvas) {
        for edge in self.edges.iter() {
            edge.draw(canvas);
        }
    }
}

impl From<Vec<Point>> for Shape {
    fn from(value: Vec<Point>) -> Self {
        let edges = value
            .iter()
            .zip(value.iter().skip(1))
            .map(|(a, b)| Edge::new(a.x, a.y, b.x, b.y, Color::Red))
            .collect::<Vec<_>>();
        Self { edges }
    }
}

impl From<Vec<Edge>> for Shape {
    fn from(value: Vec<Edge>) -> Self {
        Self { edges: value }
    }
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Edge {
    pub from: Point,
    pub to: Point,
    attr: Attributes,
}

impl Edge {
    pub fn color(&self) -> Color {
        self.attr
            .attributes
            .iter()
            .map(|a| match a {
                Attribute::Color(c) => c,
            })
            .next()
            .copied()
            .unwrap_or(Color::Black)
    }

    pub fn new(x1: f32, y1: f32, x2: f32, y2: f32, color: Color) -> Self {
        Self {
            from: Point::new(x1, y1),
            to: Point::new(x2, y2),
            attr: Attributes::default().push(Attribute::Color(color)),
        }
    }

    pub fn new_from_points(from: Point, to: Point, color: Color) -> Self {
        Self {
            from,
            to,
            attr: Attributes::default().push(Attribute::Color(color)),
        }
    }

    pub fn scale(&self, factor: f32) -> Edge {
        Edge {
            from: self.from.scale(factor),
            to: self.to.scale(factor),
            attr: self.attr.clone(),
        }
    }
}

impl Bound for &Edge {
    fn boundaries(self) -> (Point, Point) {
        (
            Point {
                x: self.from.x.min(self.to.x),
                y: self.from.y.min(self.to.y),
            },
            Point {
                x: self.from.x.max(self.to.x),
                y: self.from.y.max(self.to.y),
            },
        )
    }
}

impl Translate for Edge {
    fn translate(&mut self, dx: f32, dy: f32) {
        self.from.translate(dx, dy);
        self.to.translate(dx, dy);
    }
}

impl Draw for Edge {
    fn draw(&self, canvas: &mut Canvas) {
        let color = self.color();

        if color.as_rgba().3 == 0 {
            return;
        }

        let x1 = self.from.x as i32;
        let x2 = self.to.x as i32;
        let y1 = self.from.y as i32;
        let y2 = self.to.y as i32;

        let dx = x2 - x1;
        let dy = y2 - y1;

        if dx == 0 {
            let start_y = y1.min(y2) as usize;
            for y in start_y..start_y + dy.unsigned_abs() as usize + 1 {
                canvas.set(x1 as usize, y, color)
            }
            return;
        }

        let slope = dy as f32 / dx as f32;

        if dx > 0 {
            for step in 0..(dx + 1) as usize {
                let x = x1 as usize + step;
                let y = (self.from.y + (step as f32 * slope)) as usize;
                canvas.set(x, y, color)
            }
        } else {
            for x in 0..(dx.abs() + 1) {
                let y = (self.from.y - (x as f32 * slope)) as usize;
                let x = x1 as usize - x as usize;
                canvas.set(x, y, color)
            }
        }
    }
}

#[derive(Default, Debug, Clone, Eq, PartialEq, Hash)]
struct Attributes {
    attributes: Vec<Attribute>,
}

impl Attributes {
    fn push(mut self, attr: Attribute) -> Self {
        self.attributes.push(attr);
        self
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum Attribute {
    Color(Color),
}

#[derive(Debug, Copy, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    const MIN: Point = Point {
        x: f32::MIN,
        y: f32::MIN,
    };
    const MAX: Point = Point {
        x: f32::MAX,
        y: f32::MAX,
    };

    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn add(&self, dx: f32, dy: f32) -> Self {
        Point {
            x: self.x + dx,
            y: self.y + dy,
        }
    }

    fn top_left(&self, other: &Self) -> Point {
        Point {
            x: self.x.min(other.x),
            y: self.y.min(other.y),
        }
    }

    fn bottom_right(&self, other: &Self) -> Point {
        Point {
            x: self.x.max(other.x),
            y: self.y.max(other.y),
        }
    }

    fn scale(&self, factor: f32) -> Point {
        Self {
            x: (self.x * factor).round(),
            y: (self.y * factor).round(),
        }
    }

impl From<iced::Point> for Point {
    fn from(value: iced::Point) -> Self {
        Self {
            x: value.x,
            y: value.y,
        }
    }
}

impl Translate for Point {
    fn translate(&mut self, dx: f32, dy: f32) {
        self.x += dx;
        self.y += dy;
    }
}

impl Draw for Point {
    fn draw(&self, canvas: &mut Canvas) {
        canvas.set(self.x as usize, self.y as usize, Color::Black);
    }
}

/// g, b, b, alpha (true=transparent)
pub type RgbaColor = (u8, u8, u8, u8);

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[allow(unused)]
pub enum Color {
    Transparent,
    White,
    #[default]
    Black,
    Red,
    Green,
    Blue,
    Yellow,
    Magenta,
    Cyan,
    Custom(RgbaColor),
}

impl Color {
    pub fn as_rgba(&self) -> RgbaColor {
        match self {
            Color::Transparent => (0, 0, 0, 0),
            Color::White => (255, 255, 255, 255),
            Color::Black => (0, 0, 0, 255),
            Color::Red => (255, 0, 0, 255),
            Color::Green => (0, 255, 0, 255),
            Color::Blue => (0, 0, 255, 255),
            Color::Yellow => (255, 255, 0, 255),
            Color::Magenta => (255, 0, 255, 255),
            Color::Cyan => (0, 255, 255, 255),
            Color::Custom(c) => *c,
        }
    }

    pub fn is_transparent(&self) -> bool {
        matches!(self, Color::Transparent)
    }
}

impl TryFrom<&str> for Color {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "transparent" => Ok(Color::Transparent),
            "white" => Ok(Color::White),
            "black" => Ok(Color::Black),
            "red" => Ok(Color::Red),
            "green" => Ok(Color::Green),
            "blue" => Ok(Color::Blue),
            "yellow" => Ok(Color::Yellow),
            "magenta" => Ok(Color::Magenta),
            "cyan" => Ok(Color::Cyan),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_from_points() {
        let shape = Shape::from(vec![
            Point::new(0., 0.),
            Point::new(0., 1.),
            Point::new(1., 1.),
        ]);
        assert_eq!(shape.edges.len(), 2);
    }
}
