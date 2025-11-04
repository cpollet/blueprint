use crate::domain::Color;
use crate::lexer::{Span, Spanned, Token, lexer};
use ariadne::{Label, Report, ReportKind, sources};
use chumsky::input::ValueInput;
use chumsky::prelude::*;
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Hash)]
pub enum Coord<'s> {
    Absolute(i32, i32, Option<&'s str>),
    Relative(i32, i32, Option<&'s str>),
    Reference(&'s str),
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub enum CommandKind<'s> {
    Move(Coord<'s>),
    Draw(Coord<'s>, Color),
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Command<'s> {
    pub kind: CommandKind<'s>,
    pub src_index: usize,
}

pub fn parse<'s>(src: &'s str, filename: &Path) -> Vec<Vec<Command<'s>>> {
    let (tokens, lexer_errors) = lexer().parse(src).into_output_errors();
    let tokens = tokens.unwrap_or_default();

    let (coords, parser_errors) = parser()
        //.map_with(|shapes, e| (shapes, e.span()))
        .parse(
            tokens
                .as_slice()
                .map((src.len()..src.len()).into(), |t| (&t.node, &t.span)),
        )
        .into_output_errors();

    if !(lexer_errors.is_empty() && parser_errors.is_empty()) {
        lexer_errors
            .into_iter()
            .map(|e| e.map_token(|c| c.to_string()))
            .chain(
                parser_errors
                    .into_iter()
                    .map(|e| e.map_token(|tok| tok.to_string())),
            )
            .for_each(|e| {
                Report::build(
                    ReportKind::Error,
                    (filename.display().to_string(), e.span().into_range()),
                )
                .with_config(ariadne::Config::new().with_index_type(ariadne::IndexType::Byte))
                .with_message(e.to_string())
                .with_label(
                    Label::new((filename.display().to_string(), e.span().into_range()))
                        .with_message(e.reason().to_string())
                        .with_color(ariadne::Color::Red),
                )
                .with_labels(e.contexts().map(|(label, span)| {
                    Label::new((filename.display().to_string(), span.into_range()))
                        .with_message(format!("while parsing this {label}"))
                        .with_color(ariadne::Color::Yellow)
                }))
                .finish()
                .print(sources([(filename.display().to_string(), src)]))
                .unwrap()
            });
    }

    coords.unwrap_or_default()
}

fn parser<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Vec<Vec<Command<'src>>>, extra::Err<Rich<'tokens, Token<'src>, Span>>> + Clone
where
    I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
{
    let move_command = just(Token::Move)
        .ignore_then(coord())
        .map_with(|coord, e| Command {
            kind: CommandKind::Move(coord.node),
            src_index: (e.span() as Span).start,
        });

    let draw_command =
        edge_attributes()
            .or_not()
            .then(coord())
            .validate(|(attrs, coord), extra, emitter| {
                let mut attrs = attrs.unwrap_or_default();

                let color = match attrs.remove("color") {
                    None => Color::default(),
                    Some(color) => match Color::try_from(color.node) {
                        Ok(color) => color,
                        Err(_) => {
                            emitter.emit(Rich::custom(
                                color.span,
                                format!("`{color}` is not a known color.", color = color.node),
                            ));
                            Color::default()
                        }
                    },
                };

                Command {
                    kind: CommandKind::Draw(coord.node, color),
                    src_index: coord.span.start,
                }
            });

    let command = choice((move_command, draw_command));

    // { command, ... }
    let commands = command
        .repeated()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::OpenCurly), just(Token::CloseCurly));

    // { command, ... } ...
    commands.repeated().collect::<Vec<_>>()
}

/// Parses a potentially empty list of key/value pairs of the following form:
/// `[ key : value , ... ]`. A training comma is allowed.
fn edge_attributes<'tokens, 'src: 'tokens, I>() -> impl Parser<
    'tokens,
    I,
    HashMap<&'src str, Spanned<&'src str>>,
    extra::Err<Rich<'tokens, Token<'src>, Span>>,
> + Clone
where
    I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
{
    let ident = select! {
        Token::Ident(t) => t,
    }
    .labelled("ident");

    let edge_attr = ident
        .then_ignore(just(Token::Colon))
        .then(ident.map_with(|i, e| Spanned {
            node: i,
            span: e.span(),
        }));

    let edge_attrs = edge_attr
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<HashMap<_, _>>();

    edge_attrs.delimited_by(just(Token::OpenSquare), just(Token::CloseSquare))
}

/// Parses any of the following:
///  * `x,y` optionally followed by `#tag` into `Coord::Relative(x, y, "tag")`
///  * `@x,y` optionally followed by `#tag` into `Coord::Absolute(x, y, "tag")`
///  * `@#tag` into `Coord::Reference("tag")`
fn coord<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Spanned<Coord<'src>>, extra::Err<Rich<'tokens, Token<'src>, Span>>> + Clone
where
    I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
{
    let num = select! {
        Token::Num(n) => n,
    }
    .labelled("number");
    let tag = select! {
        Token::Tag(t) => t,
    }
    .labelled("tag");

    let num_pair = num.then_ignore(just(Token::Comma)).then(num);
    let coord_rel = num_pair
        .clone()
        .then(tag.or_not())
        .map(|((x, y), t)| Coord::Relative(x, y, t));
    let coord_abs = just(Token::At)
        .ignore_then(num_pair)
        .then(tag.or_not())
        .map(|((x, y), t)| Coord::Absolute(x, y, t));
    let coord_ref = just(Token::At).ignore_then(tag).map(Coord::Reference);

    choice((coord_rel, coord_abs, coord_ref)).map_with(|c, e| Spanned {
        node: c,
        span: e.span(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser() {
        let src = "{ move @0,0 #p0 0,5 5,5 5,0 [color:blue] @#p0 }";
        let tokens = lexer().parse(src).unwrap();
        let res = parser()
            .parse(
                tokens
                    .as_slice()
                    .map((src.len()..src.len()).into(), |t| (&t.node, &t.span)),
            )
            .unwrap();
        assert_eq!(
            res[0],
            vec![
                Command {
                    kind: CommandKind::Move(Coord::Absolute(0, 0, Some("p0"))),
                    src_index: 2,
                },
                Command {
                    kind: CommandKind::Draw(Coord::Relative(0, 5, None), Color::Black),
                    src_index: 16,
                },
                Command {
                    kind: CommandKind::Draw(Coord::Relative(5, 5, None), Color::Black),
                    src_index: 20,
                },
                Command {
                    kind: CommandKind::Draw(Coord::Relative(5, 0, None), Color::Black),
                    src_index: 24,
                },
                Command {
                    kind: CommandKind::Draw(Coord::Reference("p0"), Color::Blue),
                    src_index: 41,
                },
            ]
        );
    }
}
