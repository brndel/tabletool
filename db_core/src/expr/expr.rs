use std::sync::Arc;

use bytepack::PackFormat;

use crate::{
    expr::{
        DidYouMeanHint, EvalCtx,
        error::EvalErr,
        op::{BinaryOp, UnaryOp},
        ty_ctx::TyCtx,
    },
    named::Named,
    ty::{FieldTy, Ty},
    value::{FieldValue, Value},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Literal(FieldValue),
    BinaryOp {
        a: Box<Self>,
        op: BinaryOp,
        b: Box<Self>,
    },
    UnaryOp {
        op: UnaryOp,
        value: Box<Self>,
    },
    FieldAccess {
        value: Box<Self>,
        field: Arc<str>,
    },
    TableAccess {
        name: Arc<str>,
    },
    FnCall {
        name: Arc<str>,
        args: Vec<Self>,
    },
}

impl Expr {
    pub fn ty(&self, ctx: &TyCtx) -> Option<Ty> {
        match self {
            Expr::Literal(value) => Some(Ty::Field(value.ty())),
            Expr::BinaryOp { a, op, b } => {
                let a = a.ty(ctx)?;
                let b = b.ty(ctx)?;

                match (a, b) {
                    (Ty::Field(a), Ty::Field(b)) => Some(Ty::Field(op.ty(&a, &b)?)),
                    _ => None,
                }
            }
            Expr::UnaryOp { op, value } => {
                let value = value.ty(ctx)?;

                match value {
                    Ty::Field(value) => Some(Ty::Field(op.ty(&value)?)),
                    _ => None,
                }
            }
            Expr::FieldAccess { value, field } => {
                let value = value.ty(ctx)?;

                if let Ty::Table(table) = value {
                    let field = table.value.field(field.as_ref())?;

                    Some(Ty::Field(field.ty.clone()))
                } else {
                    None
                }
            }
            Expr::TableAccess { name } => {
                let table = ctx.tables.get(name.as_ref())?;

                Some(Ty::Table(Named {
                    name: name.clone(),
                    value: table.clone(),
                }))
            }
            Expr::FnCall { name, args } => match name.as_ref() {
                "now" if args.is_empty() => Some(Ty::Field(FieldTy::Timestamp)),
                _ => None,
            },
        }
    }

    pub fn eval(&self, ctx: &EvalCtx) -> Result<Value, EvalErr> {
        match self {
            Expr::Literal(value) => Ok(Value::Field(value.clone())),
            Expr::BinaryOp { a, op, b } => {
                let a = a.eval(ctx)?;
                let b = b.eval(ctx)?;

                op.eval(a, b).map(Into::into)
            }
            Expr::UnaryOp { op, value } => {
                let value = value.eval(ctx)?;

                op.eval(value)
            }
            Expr::FieldAccess { value, field } => {
                let value = value.eval(ctx)?;

                match value {
                    Value::Record { table, record } => {
                        let field =
                            table
                                .value
                                .field(field)
                                .ok_or_else(|| EvalErr::UnknownField {
                                    name: field.clone(),
                                    table_name: table.name.clone(),
                                })?;

                        record
                            .get_field(field)
                            .map(Into::into)
                            .ok_or(EvalErr::Bytepack)
                    }
                    v => Err(EvalErr::MissmatchedTypes {
                        found: v.ty(),
                        expected: None,
                    }),
                }
            }
            Expr::TableAccess { name } => {
                let (Some(record), Some(table)) = (ctx.records.get(name), ctx.tables.get(name))
                else {
                    let table_with_field_name = ctx
                        .tables
                        .iter()
                        .filter_map(|(table_name, table)| {
                            if ctx.records.contains_key(table_name) && table.has_field(name) {
                                Some(table_name.clone())
                            } else {
                                None
                            }
                        })
                        .next();

                    let hint = match table_with_field_name {
                        Some(table_name) => DidYouMeanHint::TableWithField {
                            table_name,
                            field_name: name.clone(),
                        },
                        None => DidYouMeanHint::None,
                    };

                    return Err(EvalErr::UnknownTable {
                        name: name.clone(),
                        did_you_mean_hint: hint,
                    });
                };

                Ok(Value::Record {
                    table: Named {
                        name: name.clone(),
                        value: table.clone(),
                    },
                    record: record.clone(),
                })
            }
            Expr::FnCall { name, args } => match name.as_ref() {
                "now" if args.is_empty() => Ok(FieldValue::Timestamp(ctx.now).into()),
                "str_len" => {
                    if args.len() != 1 {
                        return Err(EvalErr::InvalidFunctionArgCount {
                            name: name.clone(),
                            found: args.len(),
                            expected: 1,
                        });
                    }
                    let arg = args[0].eval(ctx)?;

                    let Value::Field(FieldValue::Text(input)) = arg else {
                        return Err(EvalErr::MissmatchedTypes {
                            found: arg.ty(),
                            expected: Some(FieldTy::Text.into()),
                        });
                    };

                    return Ok(FieldValue::Int(input.len() as _).into());
                }
                _ => Err(EvalErr::UnknownFunction { name: name.clone() }),
            },
        }
    }
}
