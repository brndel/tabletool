use std::str::FromStr;

use db_core::expr::{CompareOp, EqOp, LogicOp};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Token<'src> {
    Keyword(Keyword),
    Ident(&'src str),
    Op(Op),
    Number(&'src str),
    StringLiteral(&'src str),
    SpecialLiteral { tag: &'src str, content: &'src str },
    Separator(Separator),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Keyword {
    Query,
    Where,
    GroupBy,
    GroupExtra,
    True,
    False,
}

impl FromStr for Keyword {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "query" => Ok(Self::Query),
            "where" => Ok(Self::Where),
            "group_by" => Ok(Self::GroupBy),
            "group_extra" => Ok(Self::GroupBy),
            "true" => Ok(Self::True),
            "false" => Ok(Self::False),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Separator {
    Dot,
    Arrow,
    Comma,
    Colon,
    ParenOpen,
    ParenClose,
    BracketOpen,
    BracketClose,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    Plus,
    Minus,
    Mul,
    Div,
    LogicNot,
    Compare(CompareOp),
    Eq(EqOp),
    Logic(LogicOp),
}
