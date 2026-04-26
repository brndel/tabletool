use std::str::FromStr;

use db_core::{expr::{CompareOp, EqOp, LogicOp}, ty::FieldTy};

#[derive(Debug, Clone, PartialEq)]
pub enum Token<'src> {
    Keyword(Keyword),
    Ident(&'src str),
    Op(Op),
    Number(&'src str),
    StringLiteral(&'src str),
    SpecialLiteral { tag: &'src str, content: &'src str },
    Separator(Separator),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Keyword {
    Query,
    Where,
    GroupBy,
    GroupExtra,
    True,
    False,
    Let,
    FieldTy(FieldTy),
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
            "let" => Ok(Self::Let),
            "i32" => Ok(Self::FieldTy(FieldTy::IntI32)),
            "bool" => Ok(Self::FieldTy(FieldTy::Bool)),
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
    Semicolon,
    ParenOpen,
    ParenClose,
    BracketOpen,
    BracketClose,
    BraceOpen,
    BraceClose,
    Bar,
    Assign
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
