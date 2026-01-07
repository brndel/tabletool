use chumsky::{
    Parser,
    error::Rich,
    extra,
    pratt::{infix, left, prefix},
    prelude::{just, recursive},
    select,
    span::SimpleSpan,
};
use db_core::{
    expr::{BinaryOp, Expr, MathOp, UnaryOp},
    value::Value,
};

use db_core::query::Query;

use crate::token::{Keyword, Op, Separator, Token};

pub fn parser<'token, 'src: 'token>() -> impl Parser<
    'token,
    &'token [Token<'src>],
    Query,
    extra::Err<Rich<'token, Token<'src>, SimpleSpan>>,
> {
    let ident = select! {
        Token::Ident(ident) => ident
    };

    let expr = parse_expr();

    let filter = just(Token::Keyword(Keyword::Where)).ignore_then(expr);

    just(Token::Keyword(Keyword::Query))
        .ignore_then(ident)
        .then(filter.or_not())
        .map(|(name, filter)| Query {
            table_name: name.into(),
            filter,
        })
}

pub fn parse_expr<'token, 'src: 'token>()
-> impl Parser<'token, &'token [Token<'src>], Expr, extra::Err<Rich<'token, Token<'src>, SimpleSpan>>>
+ Clone {
    recursive(|expr| {
        let num = select! {
            Token::Number(num) => num,
        }
        .try_map(|num, span| {
            if let Ok(value) = num.parse() {
                Ok(Value::Int(value))
            } else {
                Err(Rich::custom(span, "Invalid integer"))
            }
        });

        let boolean = select! {
            Token::Keyword(Keyword::True) => Value::Bool(true),
            Token::Keyword(Keyword::False) => Value::Bool(false),
        };

        let table_ident = select! {
            Token::Ident(ident) => Expr::TableAccess { name: ident.into() }
        };

        let literal = num.or(boolean).map(Expr::Literal).or(table_ident);

        let atom = literal.or(expr.delimited_by(
            just(Token::Separator(Separator::ParenOpen)),
            just(Token::Separator(Separator::ParenClose)),
        ));

        let field_access = atom.foldl(
            just(Token::Separator(Separator::Dot))
                .ignore_then(select! {
                    Token::Ident(ident) => ident
                })
                .repeated(),
            |value, field| Expr::FieldAccess {
                value: Box::new(value),
                field: field.into(),
            },
        );

        let unary_op = select! {
            Token::Op(Op::Minus) => UnaryOp::Negate,
            Token::Op(Op::LogicNot) => UnaryOp::LogicNot,
        };

        let product_op = select! {
            Token::Op(Op::Mul) => BinaryOp::Math(MathOp::Mul),
            Token::Op(Op::Div) => BinaryOp::Math(MathOp::Div),
        };

        let sum_op = select! {
            Token::Op(Op::Plus) => BinaryOp::Math(MathOp::Add),
            Token::Op(Op::Minus) => BinaryOp::Math(MathOp::Sub),
        };

        let compare_op = select! {
            Token::Op(Op::Compare(op)) => BinaryOp::Compare(op),
        };

        let logic_op = select! {
            Token::Op(Op::Logic(op)) => BinaryOp::Logic(op),
        };

        let eq_op = select! {
            Token::Op(Op::Eq(op)) => BinaryOp::Eq(op),
        };

        // A lambda function does not work here because "implementation of `Fn` is not general enough"
        macro_rules! binary_fold {
            () => {
                |a, op, b, _extra| Expr::BinaryOp {
                    a: Box::new(a),
                    op,
                    b: Box::new(b),
                }
            };
        }

        let ops = field_access.pratt((
            prefix(10, unary_op, |op, value, _extra| Expr::UnaryOp {
                op,
                value: Box::new(value),
            }),
            infix(left(9), product_op, binary_fold!()),
            infix(left(8), sum_op, binary_fold!()),
            infix(left(7), compare_op, binary_fold!()),
            infix(left(6), logic_op, binary_fold!()),
            infix(left(5), eq_op, binary_fold!()),
        ));

        ops
    })
}
