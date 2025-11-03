use ariadne::{Color, Label, Report, ReportKind, sources};
use chumsky::input::ValueInput;
use chumsky::prelude::*;
use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::path::Path;

pub type Span = SimpleSpan;

#[derive(Clone, Debug, PartialEq)]
pub struct Spanned<T: Clone + Debug + PartialEq> {
    pub node: T,
    pub span: Span,
}

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

#[derive(Clone, Debug, PartialEq)]
enum Token<'src> {
    Num(i32),
    Ident(&'src str),
    Shape,
    Tag(&'src str),
    At,
    Comma,
    Colon,
    OpenCurly,
    CloseCurly,
    OpenSquare,
    CloseSquare,
}

impl Display for Token<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Num(n) => write!(f, "{n}"),
            Token::Ident(ident) => write!(f, "{ident}"),
            Token::Shape => write!(f, "shape"),
            Token::Tag(ident) => write!(f, "#{ident}"),
            Token::At => write!(f, "@"),
            Token::Comma => write!(f, ","),
            Token::Colon => write!(f, ":"),
            Token::OpenCurly => write!(f, "{{"),
            Token::CloseCurly => write!(f, "}}"),
            Token::OpenSquare => write!(f, "["),
            Token::CloseSquare => write!(f, "]"),
        }
    }
}

fn lexer<'src>()
-> impl Parser<'src, &'src str, Vec<Spanned<Token<'src>>>, extra::Err<Rich<'src, char, Span>>> {
    let num = just('-')
        .or_not()
        .map(|t| t.map(|_| -1).unwrap_or(1))
        .then(text::int(10).to_slice().from_str().unwrapped())
        .map(|(a, b): (i32, i32)| Token::Num(a * b));

    let ident = text::ascii::ident().map(|ident: &str| match ident {
        "shape" => Token::Shape,
        _ => Token::Ident(ident),
    });

    let tag = just('#')
        .ignore_then(
            any()
                .filter(|c: &char| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                .repeated()
                .to_slice(),
        )
        .map(Token::Tag);

    let comma = just(',').map(|_| Token::Comma);
    let colon = just(':').map(|_| Token::Colon);
    let at = just('@').map(|_| Token::At);
    let open_curly = just('{').map(|_| Token::OpenCurly);
    let close_curly = just('}').map(|_| Token::CloseCurly);
    let open_square = just('[').map(|_| Token::OpenSquare);
    let close_square = just(']').map(|_| Token::CloseSquare);

    let token = choice((
        num,
        ident,
        comma,
        colon,
        tag,
        at,
        open_curly,
        close_curly,
        open_square,
        close_square,
    ));

    let comment = just("//")
        .then(any().and_is(just('\n').not()).repeated())
        .padded();

    token
        .map_with(|tok, e| Spanned {
            node: tok,
            span: e.span(),
        })
        .padded_by(comment.repeated())
        .padded()
        .recover_with(skip_then_retry_until(any().ignored(), end()))
        .repeated()
        .collect()
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
    fn test_lexer() {
        assert_eq!(
            lexer().parse("123").into_result(),
            Ok(vec![Spanned {
                node: Token::Num(123),
                span: Span::from(0..3)
            }])
        );
        assert_eq!(
            lexer().parse("123 //comment").into_result(),
            Ok(vec![Spanned {
                node: Token::Num(123),
                span: Span::from(0..3)
            }])
        );
        assert_eq!(
            lexer().parse("shape").into_result(),
            Ok(vec![Spanned {
                node: Token::Shape,
                span: Span::from(0..5)
            }])
        );
        assert_eq!(
            lexer().parse("ident").into_result(),
            Ok(vec![Spanned {
                node: Token::Ident("ident"),
                span: Span::from(0..5)
            }])
        );
        assert_eq!(
            lexer().parse("123 ident shape").into_result(),
            Ok(vec![
                Spanned {
                    node: Token::Num(123),
                    span: Span::from(0..3)
                },
                Spanned {
                    node: Token::Ident("ident"),
                    span: Span::from(4..9)
                },
                Spanned {
                    node: Token::Shape,
                    span: Span::from(10..15)
                }
            ])
        );
        assert_eq!(
            lexer().parse("{}").into_result(),
            Ok(vec![
                Spanned {
                    node: Token::OpenCurly,
                    span: Span::from(0..1)
                },
                Spanned {
                    node: Token::CloseCurly,
                    span: Span::from(1..2)
                },
            ])
        );
        assert_eq!(
            lexer().parse("#my_tag").into_result(),
            Ok(vec![Spanned {
                node: Token::Tag("my_tag"),
                span: Span::from(0..7)
            }])
        );
        assert_eq!(
            lexer().parse("#-my-tag").into_result(),
            Ok(vec![Spanned {
                node: Token::Tag("-my-tag"),
                span: Span::from(0..8)
            }])
        );
        assert_eq!(
            lexer().parse("#12").into_result(),
            Ok(vec![Spanned {
                node: Token::Tag("12"),
                span: Span::from(0..3)
            }])
        );
    }

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
