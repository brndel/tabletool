use std::str::FromStr;

use chumsky::{
    IterParser, Parser,
    error::Rich,
    extra,
    prelude::{choice, just, none_of},
    span::SimpleSpan,
    text::{ident, whitespace},
};
use db_core::expr::{CompareOp, EqOp, LogicOp, Spanned};

use crate::{token::{Keyword, Op, Separator, Token}};

pub fn lexer<'src>()
-> impl Parser<'src, &'src str, Vec<Spanned<Token<'src>>>, extra::Err<Rich<'src, char, SimpleSpan>>> {
    let op = choice([
        just("==").to(Op::Eq(EqOp::Eq)),
        just("!=").to(Op::Eq(EqOp::Neq)),
        just("<=").to(Op::Compare(CompareOp::LessEq)),
        just("<").to(Op::Compare(CompareOp::Less)),
        just(">=").to(Op::Compare(CompareOp::GreaterEq)),
        just(">").to(Op::Compare(CompareOp::Greater)),
        just("+").to(Op::Plus),
        just("-").to(Op::Minus),
        just("*").to(Op::Mul),
        just("/").to(Op::Div),
        just("!").to(Op::LogicNot),
        just("&&").to(Op::Logic(LogicOp::And)),
        just("||").to(Op::Logic(LogicOp::Or)),
    ]).map(|op| Token::Op(op));

    let separator = choice([
        just("=>").to(Separator::Arrow),
        just(".").to(Separator::Dot),
        just(",").to(Separator::Comma),
        just(":").to(Separator::Colon),
        just("(").to(Separator::ParenOpen),
        just(")").to(Separator::ParenClose),
        just("[").to(Separator::BracketOpen),
        just("]").to(Separator::BracketClose),
        just("{").to(Separator::BraceOpen),
        just("}").to(Separator::BraceClose),
        just("|").to(Separator::Bar)
    ])
    .map(|op| Token::Separator(op));

    // Num with float is currently not supported, because db currently does not have f32/f64 data types
    // let num = chumsky::text::digits(10)
    //     .then(just('.').then(digits(10)).or_not())
    //     .to_slice()
    //     .map(|slice| Token::Number(slice));

    let num = chumsky::text::int(10)
        .to_slice()
        .map(|slice| Token::Number(slice));

    let string_escape = just('\\').then(choice([just('\\'), just('"')])).ignored();

    let string_content = none_of("\\\"")
        .ignored()
        .or(string_escape)
        .repeated()
        .to_slice()
        .map(|s| Token::StringLiteral(s));

    let string_literal = string_content.delimited_by(just('"'), just('"'));

    let special_literal = ident()
        .then(
            none_of("'")
                .repeated()
                .to_slice()
                .delimited_by(just('\''), just('\'')),
        )
        .map(|(tag, content)| Token::SpecialLiteral { tag, content });

    let raw_ident = just("r#").ignore_then(ident()).map(Token::Ident);

    let ident = ident().map(|ident| {
        if let Ok(keyword) = Keyword::from_str(ident) {
            Token::Keyword(keyword)
        } else {
            Token::Ident(ident)
        }
    });

    let ident = raw_ident.or(ident);

    let token = choice((op, separator, special_literal, ident, num, string_literal)).map_with(|token, extra| Spanned::new(extra.span(), token));

    token.padded_by(whitespace()).repeated().collect()
}
