mod lexer;
mod parser;
mod token;

use chumsky::Parser;
use chumsky::error::Rich;
use chumsky::input::Input;
use chumsky::span::SimpleSpan;
use db_core::expr::{Expr, Spanned};

use db_core::query::Query;

#[derive(Debug, PartialEq)]
pub struct ParseError {
    err: Rich<'static, String, SimpleSpan>,
    kind: ParseErrorKind,
}

#[derive(Debug, PartialEq)]
pub enum ParseErrorKind {
    Lexer,
    Parser,
}

impl ParseError {
    pub fn span(&self) -> &SimpleSpan {
        self.err.span()
    }

    pub fn content(&self) -> String {
        let kind_str = match self.kind {
            ParseErrorKind::Lexer => "LEX",
            ParseErrorKind::Parser => "PARSE",
        };

        format!("{}: {}", kind_str, self.err.reason())
    }
}

pub fn parse<'a>(query: &'a str) -> (Option<Query>, Vec<ParseError>) {
    let (tokens, lexer_errors) = lexer::lexer().parse(query).into_output_errors();

    let errors = lexer_errors.into_iter().map(|err| ParseError {
        err: err.map_token(|t| t.to_string()).into_owned(),
        kind: ParseErrorKind::Lexer,
    });

    let Some(tokens) = tokens else {
        return (None, errors.collect());
    };

    let (query, parser_errors) = parser::parser()
        .parse(tokens.as_slice().map(
            (query.len()..query.len()).into(),
            |Spanned { value, span }| (value, span),
        ))
        .into_output_errors();

    let errors = parser_errors
        .into_iter()
        .map(|err| ParseError {
            err: err.map_token(|t| format!("{:?}", t)).into_owned(),
            kind: ParseErrorKind::Parser,
        })
        .chain(errors);

    (query, errors.collect())
}

pub fn parse_expr(input: &str) -> (Option<Spanned<Expr>>, Vec<ParseError>) {
    let (tokens, lexer_errors) = lexer::lexer().parse(input).into_output_errors();

    let errors = lexer_errors.into_iter().map(|err| ParseError {
        err: err.map_token(|t| t.to_string()).into_owned(),
        kind: ParseErrorKind::Lexer,
    });

    let Some(tokens) = tokens else {
        return (None, errors.collect());
    };
    let (result, parser_errors) = parser::parse_expr()
        .parse(tokens.as_slice().map(
            (input.len()..input.len()).into(),
            |Spanned { value, span }| (value, span),
        ))
        .into_output_errors();

    let errors = parser_errors
        .into_iter()
        .map(|err| ParseError {
            err: err.map_token(|t| format!("{:?}", t)).into_owned(),
            kind: ParseErrorKind::Parser,
        })
        .chain(errors);

    (result, errors.collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dbg_parse() {
        let input = "query user where user.age > 10";

        let (query, errs) = parse(input);

        dbg!(errs);
        dbg!(query);
    }

    #[test]
    fn dbg_block_parse() {
        let input = "{
            let v = {
                let x = 10;
                x + 10
            };
        }";

        let (query, errs) = parse_expr(input);

        dbg!(errs);
        dbg!(query);
    }
}
