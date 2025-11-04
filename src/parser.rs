use crate::lexer::{Span, Spanned, Token, lexer};
use ariadne::{Color, Label, Report, ReportKind, sources};
use chumsky::input::ValueInput;
use chumsky::prelude::*;
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Hash)]
pub enum Coord<'s> {
    Absolute(i32, i32, Option<&'s str>),
    Relative(i32, i32, Option<&'s str>),
    Reference(&'s str),
}

#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub struct EdgeStart<'s> {
    pub coord: Coord<'s>,
    pub attributes: HashMap<&'s str, &'s str>,
    pub start: usize,
}

pub fn parse<'s>(src: &'s str, filename: &Path) -> Vec<Vec<EdgeStart<'s>>> {
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
                        .with_color(Color::Red),
                )
                .with_labels(e.contexts().map(|(label, span)| {
                    Label::new((filename.display().to_string(), span.into_range()))
                        .with_message(format!("while parsing this {label}"))
                        .with_color(Color::Yellow)
                }))
                .finish()
                .print(sources([(filename.display().to_string(), src)]))
                .unwrap()
            });
    }

    coords.unwrap_or_default()
}

fn parser<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Vec<Vec<EdgeStart<'src>>>, extra::Err<Rich<'tokens, Token<'src>, Span>>>
+ Clone
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

    let ident = select! {
        Token::Ident(t) => t,
    }
    .labelled("ident");

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
    let coord = choice((coord_rel, coord_abs, coord_ref)).map_with(|c, e| Spanned {
        node: c,
        span: e.span(),
    });

    let edge_attr = ident.then_ignore(just(Token::Colon)).then(ident);
    let edge_attr_list = edge_attr
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<HashMap<_, _>>();
    let edge_attrs = edge_attr_list.delimited_by(just(Token::OpenSquare), just(Token::CloseSquare));

    let node = coord
        .then(edge_attrs.or_not())
        .map(|(t, attributes)| EdgeStart {
            coord: t.node,
            start: t.span.start,
            attributes: attributes.unwrap_or_default(),
        });
    let nodes = node.repeated().collect::<Vec<_>>();

    just(Token::Shape)
        .ignore_then(nodes.delimited_by(just(Token::OpenCurly), just(Token::CloseCurly)))
        .repeated()
        .collect::<Vec<Vec<_>>>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser() {
        let src = "shape { @0,0 #p0 0,5 5,5 5,0 @#p0 }";
        let tokens = lexer().parse(src).unwrap();
        let res = parser()
            .parse(
                tokens
                    .as_slice()
                    .map((src.len()..src.len()).into(), |t| (&t.node, &t.span)),
            )
            .unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(
            res[0],
            vec![
                EdgeStart {
                    coord: Coord::Absolute(0, 0, Some("p0")),
                    start: 8,
                    attributes: HashMap::default(),
                },
                EdgeStart {
                    coord: Coord::Relative(0, 5, None),
                    start: 17,
                    attributes: HashMap::default()
                },
                EdgeStart {
                    coord: Coord::Relative(5, 5, None),
                    start: 21,
                    attributes: HashMap::default()
                },
                EdgeStart {
                    coord: Coord::Relative(5, 0, None),
                    start: 25,
                    attributes: HashMap::default()
                },
                EdgeStart {
                    coord: Coord::Reference("p0"),
                    start: 29,
                    attributes: HashMap::default()
                },
            ]
        );
    }
}
