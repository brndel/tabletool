use std::sync::Arc;

use crate::{
    expr::{
        EvalCtx,
        op::{BinaryOp, UnaryOp},
        ty_ctx::TyCtx,
    },
    named::Named,
    ty::Ty,
    value::Value,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Literal(Value),
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
}

impl Expr {
    pub fn ty(&self, ctx: &TyCtx) -> Option<Ty> {
        match self {
            Expr::Literal(value) => Some(value.ty()),
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
                    let field = table.value.fields.get(field.as_ref())?;

                    Some(Ty::Field(field.ty))
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
        }
    }

    pub fn eval(&self, ctx: &EvalCtx) -> Option<Value> {
        match self {
            Expr::Literal(value) => Some(value.clone()),
            Expr::BinaryOp { a, op, b } => {
                let a = a.eval(ctx)?;
                let b = b.eval(ctx)?;

                op.eval(a, b)
            }
            Expr::UnaryOp { op, value } => {
                let value = value.eval(ctx)?;

                op.eval(value)
            }
            Expr::FieldAccess { value, field } => {
                let value = value.eval(ctx)?;

                match value {
                    Value::Record { table, record } => {
                        let field = table.value.fields.get(field)?;

                        record.get_field(field)
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
        }
    }
}
