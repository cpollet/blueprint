mod domain;
mod parser;
mod ppm;

use crate::domain::{Blueprint, Bound, Draw, Edge, Point, Shape};
use crate::ppm::PpmImage;
use std::collections::HashMap;
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

    let src = fs::read_to_string(in_filename).expect("Failed to read file");
    let shapes = parser::parse(src.as_str(), in_filename);

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
                println!("{} at {:?}", tag, from);
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

    let canvas = Canvas::try_from(blueprint)
        .expect("Failed to convert blueprint")
        .pad(50, 50);

    PpmImage::from(&canvas)
        .write_to_file(&out_filename)
        .unwrap();
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

impl TryFrom<Blueprint<i32>> for Canvas {
    type Error = ();

    fn try_from(mut value: Blueprint<i32>) -> Result<Self, Self::Error> {
        value.translate_to_origin();

        let blueprint = Blueprint::<usize>::try_from(value)?;

        let boundaries = blueprint.boundaries();
        let (width, height) = (boundaries.1.x, boundaries.1.y);
        let mut canvas = Canvas::new(width + 1, height + 1);
        blueprint.draw(&mut canvas);

        Ok(canvas)
    }
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
