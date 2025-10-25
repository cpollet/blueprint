use crate::{Node, Shape};
use ariadne::{Color, Label, Report, ReportKind, sources};
use chumsky::input::ValueInput;
use chumsky::prelude::*;
use std::fmt::Display;
use std::fs;
use std::path::Path;

pub type Span = SimpleSpan;
pub type Spanned<T> = (T, Span);

pub fn parse<P: AsRef<Path>>(filename: P) -> Vec<Shape<i32>> {
    let src = fs::read_to_string(&filename).expect("Failed to read file");

    let (tokens, lexer_errors) = lexer().parse(src.as_str()).into_output_errors();
    let tokens = tokens.unwrap_or_default();

    let (shapes, parser_errors) = parser()
        //.map_with(|shapes, e| (shapes, e.span()))
        .parse(
            tokens
                .as_slice()
                .map((src.len()..src.len()).into(), |(t, s)| (t, s)),
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
                    (
                        filename.as_ref().display().to_string(),
                        e.span().into_range(),
                    ),
                )
                .with_config(ariadne::Config::new().with_index_type(ariadne::IndexType::Byte))
                .with_message(e.to_string())
                .with_label(
                    Label::new((
                        filename.as_ref().display().to_string(),
                        e.span().into_range(),
                    ))
                    .with_message(e.reason().to_string())
                    .with_color(Color::Red),
                )
                .with_labels(e.contexts().map(|(label, span)| {
                    Label::new((filename.as_ref().display().to_string(), span.into_range()))
                        .with_message(format!("while parsing this {label}"))
                        .with_color(Color::Yellow)
                }))
                .finish()
                .print(sources([(
                    filename.as_ref().display().to_string(),
                    src.clone(),
                )]))
                .unwrap()
            });
    }

    shapes.unwrap_or_default()
}

#[derive(Clone, Debug, PartialEq)]
enum Token<'src> {
    // Null,
    // Bool(bool),
    Num(i32),
    // Str(&'src str),
    // Op(&'src str),
    // Ctrl(char),
    Ident(&'src str),
    Shape,
    // Fn,
    // Let,
    // Print,
    // If,
    // Else,
    Comma,
    OpenCurly,
    CloseCurly,
}

impl Display for Token<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Num(n) => write!(f, "{n}"),
            Token::Ident(ident) => write!(f, "{ident}"),
            Token::Shape => write!(f, "shape"),
            Token::Comma => write!(f, ","),
            Token::OpenCurly => write!(f, "{{"),
            Token::CloseCurly => write!(f, "}}"),
        }
    }
}

fn lexer<'src>()
-> impl Parser<'src, &'src str, Vec<Spanned<Token<'src>>>, extra::Err<Rich<'src, char, Span>>> {
    let num = just('-')
        .or_not()
        .map(|a| a.map(|_| -1).unwrap_or(1))
        .then(text::int(10).to_slice().from_str().unwrapped())
        .map(|(a, b): (i32, i32)| Token::Num(a * b));

    let ident = text::ascii::ident().map(|ident: &str| match ident {
        "shape" => Token::Shape,
        _ => Token::Ident(ident),
    });

    let comma = just(",").map(|_| Token::Comma);

    let open_curly = just('{').map(|_| Token::OpenCurly);
    let close_curly = just('}').map(|_| Token::CloseCurly);

    let token = choice((num, ident, comma, open_curly, close_curly));

    let comment = just("//")
        .then(any().and_is(just('\n').not()).repeated())
        .padded();

    token
        .map_with(|tok, e| (tok, e.span()))
        .padded_by(comment.repeated())
        .padded()
        .recover_with(skip_then_retry_until(any().ignored(), end()))
        .repeated()
        .collect()
}

fn parser<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Vec<Shape<i32>>, extra::Err<Rich<'tokens, Token<'src>, Span>>> + Clone
where
    I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
{
    let num = select! {
        Token::Num(n) => n,
    }
    .labelled("number");

    let node = num
        .then_ignore(just(Token::Comma))
        .then(num)
        .map(|(x, y)| Node::new(x, y));

    let nodes = node.repeated().collect::<Vec<_>>();

    just(Token::Shape)
        .ignore_then(nodes.delimited_by(just(Token::OpenCurly), just(Token::CloseCurly)))
        .map(Shape::from)
        .repeated()
        .collect::<Vec<_>>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer() {
        assert_eq!(
            lexer().parse("123").into_result(),
            Ok(vec![(Token::Num(123), Span::from(0..3))])
        );
        assert_eq!(
            lexer().parse("123 //comment").into_result(),
            Ok(vec![(Token::Num(123), Span::from(0..3))])
        );
        assert_eq!(
            lexer().parse("shape").into_result(),
            Ok(vec![(Token::Shape, Span::from(0..5))])
        );
        assert_eq!(
            lexer().parse("ident").into_result(),
            Ok(vec![(Token::Ident("ident"), Span::from(0..5))])
        );
        assert_eq!(
            lexer().parse("123 ident shape").into_result(),
            Ok(vec![
                (Token::Num(123), Span::from(0..3)),
                (Token::Ident("ident"), Span::from(4..9)),
                (Token::Shape, Span::from(10..15))
            ])
        );
        assert_eq!(
            lexer().parse("{}").into_result(),
            Ok(vec![
                (Token::OpenCurly, Span::from(0..1)),
                (Token::CloseCurly, Span::from(1..2)),
            ])
        );
    }

    #[test]
    fn test_parser() {
        let src = "shape { 0,0 0,5 5,5 5,0 0,0 }";
        let tokens = lexer().parse(src).unwrap();
        let res = parser()
            .parse(
                tokens
                    .as_slice()
                    .map((src.len()..src.len()).into(), |(t, s)| (t, s)),
            )
            .unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].edges.len(), 4);
    }
}
