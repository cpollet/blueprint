mod parser;

use std::collections::HashMap;
use std::fmt::{Display, Formatter, Write};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::exit;
use std::{fs, io};

fn main() {
    let src = fs::read_to_string("examples/example.bp").expect("Failed to read file");
    let shapes = parser::parse(src.as_str(), "examples/example.bp");

    let mut blueprint = Blueprint::default();
    let mut points = HashMap::new();

    for edge_starts in shapes {
        if edge_starts.is_empty() {
            continue;
        }

        let mut nodes: Vec<Node<i32>> = Vec::with_capacity(edge_starts.len());
        let mut edges = Vec::with_capacity(edge_starts.len() - 1);

        for ((from, attr), to) in edge_starts
            .iter()
            .map(|i| (&i.coord, &i.attributes))
            .zip(edge_starts.iter().skip(1).map(|i| &i.coord))
        {
            let (from, tag) = match from {
                Coord::Absolute(x, y, tag) => (Node::new(*x, *y), *tag),
                Coord::Relative(x, y, tag) => (
                    nodes
                        .last()
                        .copied()
                        .map(|last| last.add(*x, *y))
                        .unwrap_or(Node::new(*y, *x)),
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
                println!("{} at {:?}", tag, from.point);
                points.insert(tag, from);
            }

            let to = match to {
                Coord::Absolute(x, y, _) => Node::new(*x, *y),
                Coord::Relative(x, y, _) => nodes
                    .last()
                    .copied()
                    .map(|last: Node<i32>| last.add(*x, *y))
                    .unwrap_or(Node::new(*y, *x)),

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

            edges.push(Edge::new(
                from.point.x,
                from.point.y,
                to.point.x,
                to.point.y,
                color,
            ));
        }

        blueprint.push(Shape::from(edges))
    }

    blueprint.translate_to_origin();
    let blueprint = TryInto::<Blueprint<usize>>::try_into(blueprint).unwrap();

    let boundaries = blueprint.boundaries();

    let (width, height) = (boundaries.1.x, boundaries.1.y);
    let mut canvas = Canvas::new(width + 1, height + 1);
    blueprint.draw(&mut canvas);

    let canvas = canvas.pad(50, 50);

    PpmImage::from(&canvas)
        .write_to_file("target/blueprint.ppm")
        .unwrap();
}

trait Bound<T> {
    fn boundaries(self) -> (Point<T>, Point<T>);
}

trait Translate {
    fn translate(&mut self, dx: i32, dy: i32);
}

trait Draw {
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

#[derive(Default, Debug)]
struct Blueprint<T: Copy> {
    shapes: Vec<Shape<T>>,
}

impl<T: Copy> Blueprint<T> {
    fn push(&mut self, shape: Shape<T>) {
        self.shapes.push(shape);
    }
}

impl Blueprint<i32> {
    fn translate_to_origin(&mut self) {
        let boundaries = self.boundaries();
        self.translate(-boundaries.0.x, -boundaries.0.y);
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

#[derive(Default, Debug, Eq, PartialEq)]
struct Shape<T: Copy> {
    edges: Vec<Edge<T>>,
}

impl<T: Copy> Shape<T> {
    fn push(&mut self, edge: Edge<T>) -> Node<T> {
        self.edges.push(edge);
        self.edges.last().expect("we pushed it").to
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

impl From<Vec<Node<i32>>> for Shape<i32> {
    fn from(value: Vec<Node<i32>>) -> Self {
        let edges = value
            .iter()
            .zip(value.iter().skip(1))
            .map(|(a, b)| Edge::new(a.point.x, a.point.y, b.point.x, b.point.y, Color::Red))
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

#[derive(Debug, Eq, PartialEq, Hash)]
struct Edge<T: Copy> {
    from: Node<T>,
    to: Node<T>,
    attr: Attributes,
}

impl Edge<i32> {
    fn new(x1: i32, y1: i32, x2: i32, y2: i32, color: Color) -> Self {
        Self {
            from: Node::new(x1, y1),
            to: Node::new(x2, y2),
            attr: Attributes::default().push(Attribute::Color(color)),
        }
    }

    fn len(&self) -> f32 {
        let dx = self.to.point.x.abs_diff(self.from.point.x) as f32;
        let dy = self.to.point.y.abs_diff(self.from.point.y) as f32;
        f32::sqrt(dx * dx + dy * dy)
    }
}

impl Edge<usize> {
    fn len(&self) -> f32 {
        let dx = self.to.point.x.abs_diff(self.from.point.x) as f32;
        let dy = self.to.point.y.abs_diff(self.from.point.y) as f32;
        f32::sqrt(dx * dx + dy * dy)
    }
}

impl Bound<i32> for &Edge<i32> {
    fn boundaries(self) -> (Point<i32>, Point<i32>) {
        (
            Point {
                x: self.from.point.x.min(self.to.point.x),
                y: self.from.point.y.min(self.to.point.y),
            },
            Point {
                x: self.from.point.x.max(self.to.point.x),
                y: self.from.point.y.max(self.to.point.y),
            },
        )
    }
}

impl Bound<usize> for &Edge<usize> {
    fn boundaries(self) -> (Point<usize>, Point<usize>) {
        (
            Point {
                x: self.from.point.x.min(self.to.point.x),
                y: self.from.point.y.min(self.to.point.y),
            },
            Point {
                x: self.from.point.x.max(self.to.point.x),
                y: self.from.point.y.max(self.to.point.y),
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
        let color = self
            .attr
            .attributes
            .iter()
            .map(|a| match a {
                Attribute::Color(c) => c,
            })
            .next()
            .copied()
            .unwrap_or(Color::Black);

        if color.as_rgba().3 {
            return;
        }

        let dx = self.to.point.x as i32 - self.from.point.x as i32;
        let dy = self.to.point.y as i32 - self.from.point.y as i32;

        if dx == 0 {
            let start_y = self.from.point.y.min(self.to.point.y);
            for y in start_y..start_y + dy.unsigned_abs() as usize + 1 {
                canvas.set(self.from.point.x, y, color)
            }
            return;
        }

        let slope = dy as f32 / dx as f32;

        if dx > 0 {
            for step in 0..(dx + 1) as usize {
                let x = self.from.point.x + step;
                let y = (self.from.point.y as f32 + (step as f32 * slope)) as usize;
                canvas.set(x, y, color)
            }
        } else {
            for x in 0..(dx.abs() + 1) as usize {
                let y = (self.from.point.y as f32 - (x as f32 * slope)) as usize;
                let x = self.from.point.x - x;
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

#[derive(Default, Debug, Eq, PartialEq, Hash)]
struct Attributes {
    attributes: Vec<Attribute>,
}

impl Attributes {
    fn push(mut self, attr: Attribute) -> Self {
        self.attributes.push(attr);
        self
    }
}

#[derive(Debug, Eq, PartialEq, Hash)]
enum Attribute {
    Color(Color),
}

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq, Hash)]
struct Node<T: Copy> {
    point: Point<T>,
}

impl Node<i32> {
    fn new(x: i32, y: i32) -> Node<i32> {
        Node {
            point: Point { x, y },
        }
    }

    fn add(&self, dx: i32, dy: i32) -> Node<i32> {
        Node {
            point: Point {
                x: self.point.x + dx,
                y: self.point.y + dy,
            },
        }
    }
}

#[derive(Debug, Eq, PartialEq, Hash)]
enum Coord<'s> {
    Absolute(i32, i32, Option<&'s str>),
    Relative(i32, i32, Option<&'s str>),
    Reference(&'s str),
}

#[derive(Debug, Eq, PartialEq)]
struct EdgeStart<'s> {
    coord: Coord<'s>,
    attributes: HashMap<&'s str, &'s str>,
}

impl Translate for Node<i32> {
    fn translate(&mut self, dx: i32, dy: i32) {
        self.point.translate(dx, dy);
    }
}

impl Draw for Node<usize> {
    fn draw(&self, canvas: &mut Canvas) {
        self.point.draw(canvas);
    }
}

impl TryFrom<Node<i32>> for Node<usize> {
    type Error = ();

    fn try_from(value: Node<i32>) -> Result<Self, Self::Error> {
        Ok(Self {
            point: value.point.try_into()?,
        })
    }
}

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq, Hash)]
struct Point<T> {
    x: T,
    y: T,
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

    fn new(x: i32, y: i32) -> Self {
        Self { x, y }
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

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[allow(unused)]
enum Color {
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
    fn as_rgba(&self) -> RgbaColor {
        match self {
            Color::Transparent => (0, 0, 0, true),
            Color::White => (255, 255, 255, false),
            Color::Black => (0, 0, 0, false),
            Color::Red => (255, 0, 0, false),
            Color::Green => (0, 255, 0, false),
            Color::Blue => (0, 0, 255, false),
            Color::Yellow => (255, 255, 0, false),
            Color::Magenta => (255, 0, 255, false),
            Color::Cyan => (0, 255, 255, false),
            Color::Custom(c) => *c,
        }
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

struct Canvas {
    width: usize,
    height: usize,
    pixels: Vec<Color>,
}

/// g, b, b, alpha (true=transparent)
type RgbaColor = (u8, u8, u8, bool);

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

struct PpmImage<'c> {
    canvas: &'c Canvas,
}

impl PpmImage<'_> {
    fn reader(&self) -> PpmImageReader<'_> {
        PpmImageReader::new(self)
    }

    fn write_to_file<P: AsRef<Path>>(&self, filename: P) -> Result<(), io::Error> {
        let mut file = File::create(filename)?;
        io::copy(&mut self.reader(), &mut file)?;
        Ok(())
    }
}

impl<'c> From<&'c Canvas> for PpmImage<'c> {
    fn from(value: &'c Canvas) -> Self {
        Self { canvas: value }
    }
}

impl Display for PpmImage<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "P3")?;
        writeln!(f, "{} {}", self.canvas.width, self.canvas.height)?;
        writeln!(f, "255")?;

        for y in 0..self.canvas.height {
            for x in 0..self.canvas.width {
                let (r, g, b, _) = self.canvas.get(x, y).as_rgba();
                write!(f, "{r} {g} {b} ",)?
            }
            writeln!(f)?;
        }

        Ok(())
    }
}

struct PpmImageReader<'c> {
    image: &'c PpmImage<'c>,
    x: usize,
    y: usize,
    buf: String,
    pos: usize,
}

impl<'c> PpmImageReader<'c> {
    const CAP: usize = 16;

    fn new(image: &'c PpmImage) -> Self {
        let mut buf = String::with_capacity(Self::CAP);
        writeln!(&mut buf, "P3").unwrap();
        writeln!(&mut buf, "{} {}", image.canvas.width, image.canvas.height).unwrap();
        writeln!(&mut buf, "255").unwrap();
        Self {
            image,
            x: 0,
            y: 0,
            buf,
            pos: 0,
        }
    }
}

impl Read for PpmImageReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let one_pixel_size = 12;
        let one_line_size = self.image.canvas.width * one_pixel_size + 1;

        if self.buf.len() < buf.len() {
            'outer: while self.y < self.image.canvas.height {
                while self.x < self.image.canvas.width {
                    if self.buf.len() + one_pixel_size > self.buf.capacity() {
                        break 'outer;
                    }

                    let (r, g, b, _) = self.image.canvas.get(self.x, self.y).as_rgba();

                    write!(&mut self.buf, "{r} {g} {b}",).map_err(io::Error::other)?;

                    if self.x < self.image.canvas.width - 1 {
                        write!(&mut self.buf, " ",).map_err(io::Error::other)?;
                    }

                    self.x += 1;
                }

                writeln!(&mut self.buf,).map_err(io::Error::other)?;

                self.x = 0;
                self.y += 1;

                if self.buf.len() + one_line_size > self.buf.capacity() {
                    break;
                }
            }
        }

        let from = &self.buf.as_bytes()[self.pos..];
        let to_copy = buf.len().min(from.len());
        if to_copy == 1 {
            buf[0] = from[0];
        } else {
            buf[..to_copy].copy_from_slice(from.split_at(to_copy).0);
        }

        self.pos += to_copy;

        if self.buf.len() <= self.pos {
            self.buf.clear();
            self.pos = 0;
        }

        debug_assert!(
            self.buf.capacity() == Self::CAP,
            "cap = {}",
            self.buf.capacity()
        );

        Ok(to_copy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_len() {
        assert_eq!(Edge::new(0, 0, 0, 0, Color::Black).len(), 0.0);
        assert_eq!(Edge::new(0, 0, 1, 0, Color::Black).len(), 1.0);
        assert_eq!(Edge::new(0, 0, 0, 1, Color::Black).len(), 1.0);
        assert!(Edge::new(0, 0, 1, 1, Color::Black).len() - 1.41 < 0.01);
    }

    #[test]
    fn shape_from_nodes() {
        let shape = Shape::from(vec![Node::new(0, 0), Node::new(0, 1), Node::new(1, 1)]);

        assert_eq!(shape.edges.len(), 2);
    }
}
