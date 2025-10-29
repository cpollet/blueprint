use crate::Canvas;
use std::slice::Iter;

pub trait Bound<T> {
    fn boundaries(self) -> (Point<T>, Point<T>);
}

trait Translate {
    fn translate(&mut self, dx: i32, dy: i32);
}

pub trait Draw {
    fn draw(&self, canvas: &mut Canvas);
}

impl<I, E> Bound<i32> for I
where
    I: Iterator<Item = E>,
    E: Bound<i32>,
{
    fn boundaries(self) -> (Point<i32>, Point<i32>) {
        self.fold(
            (Point::<i32>::MAX, Point::<i32>::MIN),
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

impl<I, E> Bound<usize> for I
where
    I: Iterator<Item = E>,
    E: Bound<usize>,
{
    fn boundaries(self) -> (Point<usize>, Point<usize>) {
        self.fold(
            (Point::<usize>::MAX, Point::<usize>::MIN),
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
pub struct Blueprint<T: Copy> {
    shapes: Vec<Shape<T>>,
}

impl<T: Copy> Blueprint<T> {
    pub fn push(&mut self, shape: Shape<T>) {
        self.shapes.push(shape);
    }

    pub fn shapes_iter(&self) -> Iter<'_, Shape<T>> {
        self.shapes.iter()
    }
}

impl Blueprint<i32> {
    pub fn translate_to_origin(&mut self) {
        let boundaries = self.boundaries();
        self.translate(-boundaries.0.x, -boundaries.0.y);
    }
}

impl Blueprint<usize> {
    pub fn scale(&self, factor: f32) -> Blueprint<usize> {
        Self {
            shapes: self
                .shapes
                .iter()
                .map(|shape| shape.scale(factor))
                .collect(),
        }
    }
}

impl Bound<i32> for &Blueprint<i32> {
    fn boundaries(self) -> (Point<i32>, Point<i32>) {
        self.shapes.iter().boundaries()
    }
}

impl Bound<usize> for &Blueprint<usize> {
    fn boundaries(self) -> (Point<usize>, Point<usize>) {
        self.shapes.iter().boundaries()
    }
}

impl Translate for Blueprint<i32> {
    fn translate(&mut self, dx: i32, dy: i32) {
        self.shapes
            .iter_mut()
            .for_each(|shape| shape.translate(dx, dy));
    }
}

impl Draw for Blueprint<usize> {
    fn draw(&self, canvas: &mut Canvas) {
        self.shapes.iter().for_each(|shape| shape.draw(canvas));
    }
}

impl TryFrom<Blueprint<i32>> for Blueprint<usize> {
    type Error = ();

    fn try_from(value: Blueprint<i32>) -> Result<Self, Self::Error> {
        let mut shapes = Vec::with_capacity(value.shapes.len());

        for shape in value.shapes.into_iter() {
            shapes.push(shape.try_into()?);
        }

        Ok(Blueprint::<usize> { shapes })
    }
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct Shape<T: Copy> {
    edges: Vec<Edge<T>>,
}

impl<T: Copy> Shape<T> {
    pub fn edges_iter(&self) -> Iter<'_, Edge<T>> {
        self.edges.iter()
    }
}

impl Shape<usize> {
    pub fn scale(&self, factor: f32) -> Shape<usize> {
        Self {
            edges: self.edges.iter().map(|edge| edge.scale(factor)).collect(),
        }
    }
}

impl Bound<i32> for &Shape<i32> {
    fn boundaries(self) -> (Point<i32>, Point<i32>) {
        self.edges.iter().boundaries()
    }
}

impl Bound<usize> for &Shape<usize> {
    fn boundaries(self) -> (Point<usize>, Point<usize>) {
        self.edges.iter().boundaries()
    }
}

impl Translate for Shape<i32> {
    fn translate(&mut self, dx: i32, dy: i32) {
        self.edges
            .iter_mut()
            .for_each(|edge| edge.translate(dx, dy));
    }
}

impl Draw for Shape<usize> {
    fn draw(&self, canvas: &mut Canvas) {
        for edge in self.edges.iter() {
            edge.draw(canvas);
        }
    }
}

impl From<Vec<Point<i32>>> for Shape<i32> {
    fn from(value: Vec<Point<i32>>) -> Self {
        let edges = value
            .iter()
            .zip(value.iter().skip(1))
            .map(|(a, b)| Edge::new(a.x, a.y, b.x, b.y, Color::Red))
            .collect::<Vec<_>>();
        Self { edges }
    }
}

impl From<Vec<Edge<i32>>> for Shape<i32> {
    fn from(value: Vec<Edge<i32>>) -> Self {
        Self { edges: value }
    }
}

impl TryFrom<Shape<i32>> for Shape<usize> {
    type Error = ();

    fn try_from(value: Shape<i32>) -> Result<Self, Self::Error> {
        let mut edges = Vec::with_capacity(value.edges.len());

        for edge in value.edges.into_iter() {
            edges.push(edge.try_into()?);
        }

        Ok(Shape::<usize> { edges })
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub struct Edge<T: Copy> {
    pub from: Point<T>,
    pub to: Point<T>,
    attr: Attributes,
}

impl<T: Copy> Edge<T> {
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
}

impl Edge<i32> {
    pub fn new(x1: i32, y1: i32, x2: i32, y2: i32, color: Color) -> Self {
        Self {
            from: Point::new(x1, y1),
            to: Point::new(x2, y2),
            attr: Attributes::default().push(Attribute::Color(color)),
        }
    }

    pub fn new_from_points(from: Point<i32>, to: Point<i32>, color: Color) -> Self {
        Self {
            from,
            to,
            attr: Attributes::default().push(Attribute::Color(color)),
        }
    }
}

impl Edge<usize> {
    pub fn scale(&self, factor: f32) -> Edge<usize> {
        Edge {
            from: self.from.scale(factor),
            to: self.to.scale(factor),
            attr: self.attr.clone(),
        }
    }
}

impl Bound<i32> for &Edge<i32> {
    fn boundaries(self) -> (Point<i32>, Point<i32>) {
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

impl Bound<usize> for &Edge<usize> {
    fn boundaries(self) -> (Point<usize>, Point<usize>) {
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

impl Translate for Edge<i32> {
    fn translate(&mut self, dx: i32, dy: i32) {
        self.from.translate(dx, dy);
        self.to.translate(dx, dy);
    }
}

impl Draw for Edge<usize> {
    fn draw(&self, canvas: &mut Canvas) {
        let color = self.color();

        if color.as_rgba().3 == 0 {
            return;
        }

        let dx = self.to.x as i32 - self.from.x as i32;
        let dy = self.to.y as i32 - self.from.y as i32;

        if dx == 0 {
            let start_y = self.from.y.min(self.to.y);
            for y in start_y..start_y + dy.unsigned_abs() as usize + 1 {
                canvas.set(self.from.x, y, color)
            }
            return;
        }

        let slope = dy as f32 / dx as f32;

        if dx > 0 {
            for step in 0..(dx + 1) as usize {
                let x = self.from.x + step;
                let y = (self.from.y as f32 + (step as f32 * slope)) as usize;
                canvas.set(x, y, color)
            }
        } else {
            for x in 0..(dx.abs() + 1) as usize {
                let y = (self.from.y as f32 - (x as f32 * slope)) as usize;
                let x = self.from.x - x;
                canvas.set(x, y, color)
            }
        }
    }
}

impl TryFrom<Edge<i32>> for Edge<usize> {
    type Error = ();

    fn try_from(value: Edge<i32>) -> Result<Self, Self::Error> {
        Ok(Edge {
            from: value.from.try_into()?,
            to: value.to.try_into()?,
            attr: value.attr,
        })
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

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub struct Point<T> {
    pub x: T,
    pub y: T,
}

impl Point<i32> {
    const MIN: Point<i32> = Point {
        x: i32::MIN,
        y: i32::MIN,
    };
    const MAX: Point<i32> = Point {
        x: i32::MAX,
        y: i32::MAX,
    };

    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn add(&self, dx: i32, dy: i32) -> Self {
        Point {
            x: self.x + dx,
            y: self.y + dy,
        }
    }

    fn top_left(&self, other: &Self) -> Point<i32> {
        Point {
            x: self.x.min(other.x),
            y: self.y.min(other.y),
        }
    }

    fn bottom_right(&self, other: &Self) -> Point<i32> {
        Point {
            x: self.x.max(other.x),
            y: self.y.max(other.y),
        }
    }
}

impl Point<usize> {
    const MIN: Point<usize> = Point {
        x: usize::MIN,
        y: usize::MIN,
    };
    const MAX: Point<usize> = Point {
        x: usize::MAX,
        y: usize::MAX,
    };

    fn top_left(&self, other: &Self) -> Point<usize> {
        Point {
            x: self.x.min(other.x),
            y: self.y.min(other.y),
        }
    }

    fn bottom_right(&self, other: &Self) -> Point<usize> {
        Point {
            x: self.x.max(other.x),
            y: self.y.max(other.y),
        }
    }

    fn scale(&self, factor: f32) -> Point<usize> {
        Self {
            x: (self.x as f64 * factor as f64).round() as usize,
            y: (self.y as f64 * factor as f64).round() as usize,
        }
    }
}

impl Translate for Point<i32> {
    fn translate(&mut self, dx: i32, dy: i32) {
        self.x += dx;
        self.y += dy;
    }
}

impl Draw for Point<usize> {
    fn draw(&self, canvas: &mut Canvas) {
        canvas.set(self.x, self.y, Color::Black);
    }
}

impl TryFrom<Point<i32>> for Point<usize> {
    type Error = ();

    fn try_from(value: Point<i32>) -> Result<Self, Self::Error> {
        if value.x < 0 || value.y < 0 {
            return Err(());
        }
        Ok(Self {
            x: value.x as usize,
            y: value.y as usize,
        })
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
        let shape = Shape::from(vec![Point::new(0, 0), Point::new(0, 1), Point::new(1, 1)]);
        assert_eq!(shape.edges.len(), 2);
    }
}
