use std::str::FromStr;

use chumsky::{
    IterParser, Parser,
    error::Rich,
    extra,
    prelude::{choice, just, none_of},
    select,
    span::SimpleSpan,
    text::{ident, whitespace},
};
use db_core::expr::{CompareOp, EqOp, LogicOp};

use crate::token::{Keyword, Op, Separator, Token};

pub fn lexer<'src>()
-> impl Parser<'src, &'src str, Vec<Token<'src>>, extra::Err<Rich<'src, char, SimpleSpan>>> {
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
    ]);

    let op = op.map(Token::Op);

    // Num with float is currently not supported, because db currently does not have f32/f64 data types
    // let num = chumsky::text::digits(10)
    //     .then(just('.').then(digits(10)).or_not())
    //     .to_slice()
    //     .map(|slice| Token::Number(slice));

    let num = chumsky::text::int(10)
        .to_slice()
        .map(|slice| Token::Number(slice));

    let string_escape = just('\\').then(choice([
        just('\\'),
        just('"'),
    ])).ignored();

    let string_content = none_of("\\\"").ignored().or(string_escape).repeated().to_slice().map(Token::StringLiteral);

    let string_literal = string_content.delimited_by(just('"'), just('"'));

    let separator = select! {
        '.' => Separator::Dot,
        ',' => Separator::Comma,
        ':' => Separator::Colon,
        '(' => Separator::ParenOpen,
        ')' => Separator::ParenClose,
    }
    .map(Token::Separator);

    let ident = ident().map(|ident| {
        if let Ok(keyword) = Keyword::from_str(ident) {
            Token::Keyword(keyword)
        } else {
            Token::Ident(ident)
        }
    });

    let token = ident.or(op).or(num).or(string_literal).or(separator);

    token.padded_by(whitespace()).repeated().collect()
}
