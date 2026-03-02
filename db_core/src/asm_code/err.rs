use std::sync::Arc;

use thiserror::Error;

use crate::{
    expr::{BinaryOp, UnaryOp},
    ty::Ty,
};

#[derive(Debug, Error)]
pub enum AsmCompileErr {
    #[error("Missmatched type: expected {expected:?} but found {found:?}")]
    MissmatchedTy { expected: Ty, found: Ty },
    #[error("Invalid types for binary op {op:?}: {a:?} {b:?}")]
    InvalidBinaryOpTy { op: BinaryOp, a: Ty, b: Ty },
    #[error("Invalid types for unary op {op:?}: {value:?}")]
    InvalidUnaryOpTy { op: UnaryOp, value: Ty },
    #[error("Field access on asm pointer")]
    FieldAccessOnAsmPointer,
    #[error("Field access on non-record iter")]
    FieldAccessOnNoneRecordIter,
    #[error("Field access on invalid ty {ty:?}")]
    FieldAccessOnInvalidTy { ty: Ty },
    #[error("Table access without field")]
    TableAccessWithoutField,
    #[error("Unkown Field '{field}' on table '{table_name}'")]
    UnkownTableField {
        field: Arc<str>,
        table_name: Arc<str>,
    },
    #[error("sum fn got wrong ty")]
    SumWrongIterThiny {
        ty: Ty
    },
    #[error("unkown fn '{fn_name}'")]
    UnkownFn {
        fn_name: String
    },
    #[error("Wrong Arg count on fn '{fn_name}': expected {expected} but found {found}")]
    WrongArgCount {
        fn_name: &'static str,
        expected: usize,
        found: usize,
    },
}
