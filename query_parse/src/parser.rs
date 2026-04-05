use chrono::DateTime;
use chumsky::{
    IterParser, Parser,
    container::Seq,
    error::Rich,
    extra,
    input::ValueInput,
    pratt::{infix, left, prefix},
    prelude::{choice, just, recursive},
    select,
    span::SimpleSpan,
};
use db_core::{
    expr::{BinaryOp, Expr, MathOp, Spanned, UnaryOp},
    named::Named,
    ulid::Ulid,
    value::FieldValue,
};

use db_core::query::Query;

use crate::token::{Keyword, Op, Separator, Token};

pub fn parser<'token, 'src: 'token, I>()
-> impl Parser<'token, I, Query, extra::Err<Rich<'token, Token<'src>, SimpleSpan>>>
where
    I: ValueInput<'token, Token = Token<'src>, Span = SimpleSpan>,
{
    let ident = select! {
        Token::Ident(ident) = e => Spanned::new(e.span(), ident)
    };

    let expr = parse_expr();

    let filter = just(Token::Keyword(Keyword::Where)).ignore_then(expr.clone());
    let group = just(Token::Keyword(Keyword::GroupBy))
        .ignore_then(expr.clone())
        .then(
            just(Token::Keyword(Keyword::GroupBy))
                .ignore_then(expr)
                .or_not(),
        );

    just(Token::Keyword(Keyword::Query))
        .ignore_then(ident)
        .then(filter.or_not())
        .then(group.or_not())
        .map(|((name, filter), group)| {
            let (group_by, group_extra) = match group {
                Some((group_by, Some(group_extra))) => (Some(group_by), Some(group_extra)),
                Some((group_by, None)) => (Some(group_by), None),
                None => (None, None),
            };

            Query {
                table_name: name.map(Into::into),
                filter,
                group_by,
                group_extra,
            }
        })
}

pub fn parse_expr<'token, 'src: 'token, I>()
-> impl Parser<'token, I, Spanned<Expr>, extra::Err<Rich<'token, Token<'src>, SimpleSpan>>> + Clone
where
    I: ValueInput<'token, Token = Token<'src>, Span = SimpleSpan>,
{
    recursive(|expr| {
        let atom = parse_atom(expr);

        // let tail_fn_call = atom.clone().foldl_with(
        //     just(Token::Separator(Separator::Dot))
        //         .ignore_then(select! {
        //             Token::Ident(ident) = e => Spanned::new(e.span(), ident.into())
        //         })
        //         .then(
        //             atom.separated_by(just(Token::Separator(Separator::Comma)))
        //                 .allow_trailing()
        //                 .collect::<Vec<_>>()
        //                 .delimited_by(
        //                     just(Token::Separator(Separator::ParenOpen)),
        //                     just(Token::Separator(Separator::ParenClose)),
        //                 ),
        //         )
        //         .repeated(),
        //     |value, (fn_name, fn_args), extra| {
        //         Spanned::new(
        //             extra.span(),
        //             Expr::FnCall {
        //                 name: fn_name,
        //                 args: std::iter::once(value).chain(fn_args).collect(),
        //             },
        //         )
        //     },
        // );

        let field_access = atom.clone().foldl_with(
            just(Token::Separator(Separator::Dot))
                .to_span()
                .then(
                    select! {
                        Token::Ident(ident) = e => Spanned::new(e.span(), ident.into())
                    }
                    .or_not(),
                )
                .then(
                    atom.separated_by(just(Token::Separator(Separator::Comma)))
                        .allow_trailing()
                        .collect::<Vec<_>>()
                        .delimited_by(
                            just(Token::Separator(Separator::ParenOpen)),
                            just(Token::Separator(Separator::ParenClose)),
                        )
                        .or_not(),
                )
                .repeated(),
            |value, ((dot_span, field), fn_args), extra| {
                let expr = match fn_args {
                    Some(fn_args) => Expr::FnCall {
                        name: field.unwrap_or_else(|| Spanned::new(dot_span, Default::default())),
                        args: std::iter::once(value).chain(fn_args).collect(),
                    },
                    None => Expr::FieldAccess {
                        value: value.map(Box::new),
                        dot_span,
                        field: field,
                    },
                };

                Spanned::new(extra.span(), expr)
            },
        );

        let unary_op = select! {
            Token::Op(Op::Minus) = e => Spanned::new(e.span(), UnaryOp::Negate),
            Token::Op(Op::LogicNot) = e => Spanned::new(e.span(), UnaryOp::LogicNot),
        };

        let product_op = select! {
            Token::Op(Op::Mul) = e => Spanned::new(e.span(), BinaryOp::Math(MathOp::Mul)),
            Token::Op(Op::Div) = e => Spanned::new(e.span(), BinaryOp::Math(MathOp::Div)),
        };

        let sum_op = select! {
            Token::Op(Op::Plus) = e => Spanned::new(e.span(), BinaryOp::Math(MathOp::Add)),
            Token::Op(Op::Minus) = e => Spanned::new(e.span(), BinaryOp::Math(MathOp::Sub)),
        };

        let compare_op = select! {
            Token::Op(Op::Compare(op)) = e => Spanned::new(e.span(), BinaryOp::Compare(op)),
        };

        let logic_op = select! {
            Token::Op(Op::Logic(op)) = e => Spanned::new(e.span(), BinaryOp::Logic(op)),
        };

        let eq_op = select! {
            Token::Op(Op::Eq(op)) = e => Spanned::new(e.span(), BinaryOp::Eq(op)),
        };

        // A lambda function does not work here because "implementation of `Fn` is not general enough"
        macro_rules! binary_fold {
            () => {
                |a: Spanned<Expr>, op: Spanned<BinaryOp>, b: Spanned<Expr>, extra| {
                    Spanned::new(
                        extra.span(),
                        Expr::BinaryOp {
                            a: Spanned::map(a, Box::new),
                            op,
                            b: Spanned::map(b, Box::new),
                        },
                    )
                }
            };
        }

        let ops = field_access.pratt((
            prefix(
                10,
                unary_op,
                |op: Spanned<UnaryOp>, value: Spanned<Expr>, extra| {
                    Spanned::new(
                        extra.span(),
                        Expr::UnaryOp {
                            op,
                            value: value.map(Box::new),
                        },
                    )
                },
            ),
            infix(left(9), product_op, binary_fold!()),
            infix(left(8), sum_op, binary_fold!()),
            infix(left(7), compare_op, binary_fold!()),
            infix(left(6), eq_op, binary_fold!()),
            infix(left(5), logic_op, binary_fold!()),
        ));

        ops
    })
}

fn parse_atom<'token, 'src: 'token, I, E>(
    expr: E,
) -> impl Parser<'token, I, Spanned<Expr>, extra::Err<Rich<'token, Token<'src>, SimpleSpan>>> + Clone
where
    I: ValueInput<'token, Token = Token<'src>, Span = SimpleSpan>,
    E: Parser<'token, I, Spanned<Expr>, extra::Err<Rich<'token, Token<'src>, SimpleSpan>>> + Clone,
{
    let fn_call = select! { Token::Ident(ident) = e => Spanned::new(e.span(), ident.into()) }
        .then(
            expr.clone()
                .separated_by(just(Token::Separator(Separator::Comma)))
                .collect()
                .delimited_by(
                    just(Token::Separator(Separator::ParenOpen)),
                    just(Token::Separator(Separator::ParenClose)),
                ),
        )
        .map_with(|(name, args), extra| Spanned::new(extra.span(), Expr::FnCall { name, args }));

    let num = select! {
        Token::Number(num) => num,
    }
    .try_map(|num, span| {
        if let Ok(value) = num.parse() {
            Ok(FieldValue::Int(value))
        } else {
            Err(Rich::custom(span, "Invalid integer"))
        }
    });

    let special_literal = select! {
        Token::SpecialLiteral { tag, content } => (tag, content)
    }
    .try_map(|(tag, content), span| match tag {
        "id" => match content.split_once(":") {
            Some((table, id)) => {
                let id = Ulid::from_string(id)
                    .map_err(|err| Rich::custom(span, format!("{:?}", err)))?;
                Ok(FieldValue::RecordId {
                    id,
                    table_name: table.into(),
                })
            }
            None => Err(Rich::custom(
                span,
                format!("id literal needs format '<table name>:<id>'"),
            )),
        },
        "dt" => match DateTime::parse_from_rfc3339(content) {
            Ok(dt) => Ok(FieldValue::Timestamp(dt.to_utc())),
            Err(err) => Err(Rich::custom(span, format!("{:?}", err))),
        },
        _ => Err(Rich::custom(span, format!("unkown literal tag '{tag}'"))),
    });

    let literal = select! {
        Token::Keyword(Keyword::True) => FieldValue::Bool(true),
        Token::Keyword(Keyword::False) => FieldValue::Bool(false),
        Token::StringLiteral(value) => FieldValue::Text(value.to_owned()),
    }
    .or(num)
    .or(special_literal)
    .map_with(|literal, extra| {
        Spanned::new(
            extra.span(),
            Expr::Literal(Spanned::new(extra.span(), literal)),
        )
    });

    let variable = select! {
        Token::Ident(ident) = e => Spanned::new(e.span(), Expr::Variable { name: Spanned::new(e.span(), ident.into()) })
    };

    let paren_expr = expr.clone().delimited_by(
        just(Token::Separator(Separator::ParenOpen)),
        just(Token::Separator(Separator::ParenClose)),
    );

    let array_expr = expr
        .clone()
        .separated_by(just(Token::Separator(Separator::Comma)))
        .allow_trailing()
        .collect()
        .delimited_by(
            just(Token::Separator(Separator::BracketOpen)),
            just(Token::Separator(Separator::BracketClose)),
        )
        .map_with(|exprs, extra| Spanned::new(extra.span(), Expr::Array(exprs)));

    let ident = select! {Token::Ident(s) => s};

    let struct_expr = ident
        .then_ignore(just(Token::Separator(Separator::Colon)))
        .then(expr.clone())
        .map(|(name, value)| value.map(|value| Named::new(name, value)))
        .separated_by(just(Token::Separator(Separator::Comma)))
        .allow_trailing()
        .collect()
        .delimited_by(
            just(Token::Separator(Separator::BraceOpen)),
            just(Token::Separator(Separator::BraceClose)),
        )
        .map_with(|fields, extra| Spanned::new(extra.span(), Expr::Struct { fields }));

    let ident = select! {Token::Ident(s) = e => Spanned::new(e.span(), s.into())};

    let lambda_fn = ident
        .separated_by(just(Token::Separator(Separator::Comma)))
        .collect()
        .delimited_by(
            just(Token::Separator(Separator::Bar)),
            just(Token::Separator(Separator::Bar)),
        )
        .then_ignore(just(Token::Separator(Separator::Arrow)))
        .then(expr)
        .map_with(|(args, body), extra| {
            Spanned::new(
                extra.span(),
                Expr::LambdaFn {
                    args,
                    body: body.map(Box::new),
                },
            )
        });

    let atom = choice((
        fn_call,
        literal,
        variable,
        paren_expr,
        array_expr,
        struct_expr,
        lambda_fn,
    ));

    return atom;
}
