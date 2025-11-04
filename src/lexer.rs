use chumsky::prelude::*;
use std::fmt::{Debug, Display};

pub type Span = SimpleSpan;

#[derive(Clone, Debug, PartialEq)]
pub struct Spanned<T: Clone + Debug + PartialEq> {
    pub node: T,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Token<'src> {
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

pub fn lexer<'src>()
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

#[cfg(test)]
mod test {
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
}