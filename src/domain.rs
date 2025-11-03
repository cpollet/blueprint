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

    pub fn find_closest_edge(&self, p: Point) -> Option<(&Edge, Point, f32)> {
        let mut closest = None;

        for shape in self.shapes.iter() {
            for edge in shape.edges.iter() {
                if edge.color == Color::Transparent {
                    continue;
                }
                if let Some((d, point)) = p.distance_to_edge(edge)
                    && d < closest.map(|(_, d, _)| d).unwrap_or(f32::INFINITY)
                {
                    closest = Some((edge, d, point))
                }
            }
        }

        closest.map(|(e, d, p)| (e, p, d))
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

impl From<Vec<Edge>> for Shape {
    fn from(value: Vec<Edge>) -> Self {
        Self { edges: value }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
#[non_exhaustive]
pub struct Edge {
    pub from: Point,
    pub to: Point,
    pub color: Color,
    pub line: usize,
}

impl Edge {
    pub fn new(x1: f32, y1: f32, x2: f32, y2: f32, color: Color, line: usize) -> Self {
        Self {
            from: Point::new(x1, y1),
            to: Point::new(x2, y2),
            color,
            line,
        }
    }

    pub fn new_from_points(from: Point, to: Point, color: Color, line: usize) -> Self {
        Self {
            from,
            to,
            color,
            line,
        }
    }

    pub fn scale(&self, factor: f32) -> Edge {
        Edge {
            from: self.from.scale(factor),
            to: self.to.scale(factor),
            color: self.color,
            line: self.line,
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
        let color = self.color;

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

    pub fn distance_to_point(&self, point: &Point) -> f32 {
        ((self.x - point.x).powf(2.) + (self.y - point.y).powf(2.)).sqrt()
    }

    pub fn distance_to_edge(&self, edge: &Edge) -> Option<(f32, Point)> {
        let point = self.closest_point_on_edge(edge);
        Some((self.distance_to_point(&point), point))
    }

    pub fn closest_point_on_edge(&self, edge: &Edge) -> Point {
        let projection = self.project_on_edge(edge);

        let edge_box = edge.boundaries();

        if projection.x < edge_box.0.x || projection.y < edge_box.0.y {
            return edge_box.0;
        }

        if projection.x > edge_box.1.x || projection.y > edge_box.1.y {
            return edge_box.1;
        }

        projection
    }

    pub fn project_on_edge(&self, edge: &Edge) -> Point {
        // http://stackoverflow.com/questions/64330618/finding-the-projection-of-a-point-onto-a-line
        let a = edge.from;
        let b = edge.to;

        let ab_dx = b.x - a.x;
        let ab_dy = b.y - a.y;

        let c = self;

        let acx = c.x - a.x;
        let acy = c.y - a.y;

        let coeff = (ab_dx * acx + ab_dy * acy) / (ab_dx * ab_dx + ab_dy * ab_dy);

        Point::new(edge.from.x + ab_dx * coeff, edge.from.y + ab_dy * coeff)
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
