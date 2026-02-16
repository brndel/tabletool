mod lexer;
mod parser;
mod token;

use chumsky::Parser;
use db_core::expr::Expr;

use db_core::query::Query;

pub fn parse<'a>(query: &'a str) -> Option<Query> {
    let tokens = lexer::lexer().parse(query).into_output()?;

    let query = parser::parser().parse(&tokens).into_output()?;

    Some(query)
}

pub fn parse_expr(input: &str) -> Option<Expr> {
    let tokens = lexer::lexer().parse(input).into_output()?;

    let result = parser::parse_expr().parse(&tokens).into_output()?;

    Some(result)
}

#[cfg(test)]
mod tests {
    use db_core::{
        expr::{BinaryOp, CompareOp, EqOp, EvalCtx, Expr, MathOp, TyCtx},
        ty::{FieldTy, Ty},
        value::{FieldValue, Value},
    };

    use super::*;

    #[test]
    fn test_parse() {
        let input = "query user where 5 < 10 * 4 + 2 == true";

        let query = parse(input).unwrap();

        let expr = Expr::BinaryOp {
            a: Box::new(Expr::BinaryOp {
                a: Box::new(Expr::Literal(FieldValue::Int(5))),
                op: BinaryOp::Compare(CompareOp::Less),
                b: Box::new(Expr::BinaryOp {
                    a: Box::new(Expr::BinaryOp {
                        a: Box::new(Expr::Literal(FieldValue::Int(10))),
                        op: BinaryOp::Math(MathOp::Mul),
                        b: Box::new(Expr::Literal(FieldValue::Int(4))),
                    }),
                    op: BinaryOp::Math(MathOp::Add),
                    b: Box::new(Expr::Literal(FieldValue::Int(2))),
                }),
            }),
            op: BinaryOp::Eq(EqOp::Eq),
            b: Box::new(Expr::Literal(FieldValue::Bool(true))),
        };

        let value = Query {
            table_name: "user".into(),
            filter: Some(expr),
        };

        assert_eq!(query, value);

        let ty_ctx = TyCtx { tables: Default::default() };
        let eval_ctx = EvalCtx::default();

        assert_eq!(
            query.filter.as_ref().and_then(|filter| filter.ty(&ty_ctx)),
            Some(Ty::Field(FieldTy::Bool))
        );
        assert_eq!(
            query.filter.as_ref().and_then(|filter| filter.eval(&eval_ctx)),
            Some(Value::Field(FieldValue::Bool(true)))
        );
    }

    #[test]
    fn dbg_parse() {
        let input = "query user where user.age > 10";

        let query = parse(input).unwrap();

        dbg!(query);
    }
}
