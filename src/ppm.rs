use crate::Canvas;
use std::fmt::Write;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io;
use std::io::Read;
use std::path::Path;

pub struct PpmImage<'c> {
    canvas: &'c Canvas,
}

impl PpmImage<'_> {
    fn reader(&self) -> PpmImageReader<'_> {
        PpmImageReader::new(self)
    }

    pub fn write_to_file<P: AsRef<Path>>(&self, filename: P) -> Result<(), io::Error> {
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
