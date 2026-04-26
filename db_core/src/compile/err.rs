use std::sync::Arc;

use thiserror::Error;

use crate::{
    expr::{BinaryOp, UnaryOp},
    ty::Ty,
};

#[derive(Debug, Error)]
pub enum CompileErr {
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
    #[error("Field access missing Name")]
    FieldAccessMissingName,
    #[error("Table access without field")]
    TableAccessWithoutField,
    #[error("Unkown Field '{field}' on table '{table_name}'")]
    UnkownTableField {
        field: Arc<str>,
        table_name: Arc<str>,
    },
    #[error("Unkown Table '{table_name}'")]
    UnkownTable { table_name: Arc<str> },
    #[error("Unkown Variable '{var}'")]
    UnkownVar { var: Arc<str> },
    #[error("sum fn got wrong ty")]
    SumNotCalledOnI32Iter { ty: Ty },
    #[error("unkown fn '{fn_name}'")]
    UnkownFn { fn_name: String },
    #[error("Wrong Arg count on fn '{fn_name}': expected {expected} but found {found}")]
    WrongArgCount {
        fn_name: &'static str,
        expected: usize,
        found: usize,
    },
    #[error("Non-Field Type {ty:?} is not allowed in struct values")]
    NonFieldTyInStruct { ty: Ty },
    #[error("Any Error, we do not care at the moment :)")]
    Anything,
    #[error("Custom Error {0}")]
    Custom(String)
}
