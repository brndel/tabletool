use std::sync::Arc;

use bytepack::PackFormat;

use crate::{
    expr::{
        EvalCtx,
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

    pub fn eval(&self, ctx: &EvalCtx) -> Option<Value> {
        match self {
            Expr::Literal(value) => Some(Value::Field(value.clone())),
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
                        let field = table.value.field(field)?;

                        record.get_field(field).map(Into::into)
                    }
                    _ => None,
                }
            }
            Expr::TableAccess { name } => {
                let record = ctx.records.get(name)?;
                let table = ctx.tables.get(name)?;

                Some(Value::Record {
                    table: Named {
                        name: name.clone(),
                        value: table.clone(),
                    },
                    record: record.clone(),
                })
            }
            Expr::FnCall { name, args } => match name.as_ref() {
                "now" if args.is_empty() => Some(FieldValue::Timestamp(ctx.now).into()),
                _ => None,
            },
        }
    }
}
