use std::{collections::BTreeMap, sync::Arc};

use bytepack::PackFormat;

use crate::{
    asm_code::{
        asm_code::{AsmCode, IntBits, Literal},
        pointer::{AsmPointer, AsmSlicePointer},
        program::Program,
    },
    defs::table::TableData,
    expr::{BinaryOp, Expr},
    named::Named,
    ty::{FieldTy, Ty},
    value::FieldValue,
};

use super::pointer::Namespace;

pub fn compile_expr(
    expr: &Expr,
    tables: &BTreeMap<Arc<str>, Arc<TableData>>,
) -> Result<Program, ()> {
    let mut builder = CodeBuilder::default();
    let ctx = Ctx {
        builder: &mut builder,
        tables,
        is_in_field_access: false,
    };

    let (_ptr, ty) = compile_expr_with_ctx(expr, ctx)?;

    Ok(builder.finish(ty))
}

struct Ctx<'a> {
    builder: &'a mut CodeBuilder,
    tables: &'a BTreeMap<Arc<str>, Arc<TableData>>,
    is_in_field_access: bool,
}

#[derive(Default)]
struct CodeBuilder {
    const_memory: Vec<u8>,
    code: Vec<AsmCode>,
    table_indices: BTreeMap<Arc<str>, u16>,
    stack_pointer: u32,
    max_stack_pointer: u32,
}

impl CodeBuilder {
    pub fn push_const(&mut self, bytes: &[u8]) -> u32 {
        let offset = self.const_memory.len() as u32;

        self.const_memory.extend_from_slice(bytes);

        return offset;
    }

    fn push_stack(&mut self, value: impl Into<Literal>) -> AsmPointer {
        let value = value.into();
        let byte_count = value.as_ref().len() as u32;
        let pointer = self.reserve_stack(byte_count);

        self.code.push(AsmCode::SetLiteral {
            target: pointer,
            value,
        });

        pointer
    }

    fn reserve_stack(&mut self, byte_count: u32) -> AsmPointer {
        let pointer = AsmPointer {
            namespace: Namespace::Stack,
            offset: self.stack_pointer,
        };
        self.stack_pointer += byte_count;
        self.max_stack_pointer = self.max_stack_pointer.max(self.stack_pointer);

        pointer
    }

    fn set_stack_pointer(&mut self, offset: u32) {
        self.stack_pointer = offset;
        self.max_stack_pointer = self.max_stack_pointer.max(self.stack_pointer);
    }

    fn stack_pointer(&self) -> u32 {
        self.stack_pointer
    }

    fn table_idx(&mut self, name: &Arc<str>) -> u16 {
        if let Some(idx) = self.table_indices.get(name) {
            *idx
        } else {
            let new_idx = self.table_indices.len() as _;
            self.table_indices.insert(name.clone(), new_idx);

            new_idx
        }
    }

    fn finish(mut self, return_ty: Ty) -> Program {
        self.code.insert(
            0,
            AsmCode::ReserveStack {
                bytes: self.max_stack_pointer,
            },
        );
        Program {
            const_memory: self.const_memory,
            code: self.code,
            table_indices: self.table_indices,
            return_ty,
        }
    }
}

fn compile_expr_with_ctx(expr: &Expr, ctx: Ctx) -> Result<(AsmPointer, Ty), ()> {
    match expr {
        Expr::Literal(field_value) => {
            let result = match field_value {
                FieldValue::Int(value) => {
                    let ptr = ctx.builder.push_stack(*value);
                    (ptr, Ty::Field(FieldTy::IntI32))
                }
                FieldValue::Bool(value) => {
                    let ptr = ctx.builder.push_stack(*value);
                    (ptr, Ty::Field(FieldTy::Bool))
                }
                FieldValue::Timestamp(value) => {
                    let ptr = ctx.builder.push_stack(*value);
                    (ptr, Ty::Field(FieldTy::Timestamp))
                }
                FieldValue::Text(value) => {
                    let offset = ctx.builder.push_const(value.as_bytes());
                    let ptr = ctx.builder.push_stack(AsmSlicePointer {
                        pointer: AsmPointer {
                            namespace: Namespace::Const,
                            offset,
                        },
                        len: value.len() as u32,
                    });
                    (ptr, Ty::Field(FieldTy::Text))
                }
                FieldValue::RecordId {
                    id: value,
                    table_name,
                } => {
                    let ptr = ctx.builder.push_stack(Literal::from(*value));
                    (
                        ptr,
                        Ty::Field(FieldTy::RecordId {
                            table_name: table_name.clone(),
                        }),
                    )
                }
            };

            Ok(result)
        }
        Expr::BinaryOp { a, op, b } => {
            let stack = ctx.builder.stack_pointer();

            let (a_ptr, a_ty) = compile_expr_with_ctx(
                &a,
                Ctx {
                    builder: ctx.builder,
                    tables: ctx.tables,
                    is_in_field_access: ctx.is_in_field_access,
                },
            )?;

            let (b_ptr, b_ty) = compile_expr_with_ctx(
                &b,
                Ctx {
                    builder: ctx.builder,
                    tables: ctx.tables,
                    is_in_field_access: ctx.is_in_field_access,
                },
            )?;

            ctx.builder.set_stack_pointer(stack);

            let result = match op {
                BinaryOp::Math(math_op) => {
                    if a_ty == Ty::Field(FieldTy::IntI32) && b_ty == Ty::Field(FieldTy::IntI32) {
                        let bits = IntBits::I32;
                        let target = ctx.builder.reserve_stack(bits.bytes());
                        ctx.builder.code.push(AsmCode::MathOp {
                            a: a_ptr,
                            b: b_ptr,
                            op: *math_op,
                            target,
                            bits,
                        });
                        (target, Ty::Field(FieldTy::IntI32))
                    } else {
                        return Err(());
                    }
                }
                BinaryOp::Logic(logic_op) => {
                    if a_ty == Ty::Field(FieldTy::Bool) && b_ty == Ty::Field(FieldTy::Bool) {
                        let target = ctx.builder.reserve_stack(1);
                        ctx.builder.code.push(AsmCode::LogicOp {
                            a: a_ptr,
                            b: b_ptr,
                            op: *logic_op,
                            target,
                        });
                        (target, Ty::Field(FieldTy::Bool))
                    } else {
                        return Err(());
                    }
                }
                BinaryOp::Compare(compare_op) => {
                    let target = if a_ty == Ty::Field(FieldTy::IntI32)
                        && b_ty == Ty::Field(FieldTy::IntI32)
                    {
                        let target = ctx.builder.reserve_stack(1);
                        ctx.builder.code.push(AsmCode::TestInt {
                            a: a_ptr,
                            b: b_ptr,
                            target,
                            bits: IntBits::I32,
                        });
                        target
                    } else {
                        return Err(());
                    };

                    ctx.builder.code.push(AsmCode::SetLiteralConditional {
                        test_result: target,
                        op: crate::asm_code::asm_code::ConditionOp::Compare(*compare_op),
                        target,
                        true_value: true.into(),
                        false_value: Some(false.into()),
                    });
                    (target, Ty::Field(FieldTy::Bool))
                }
                BinaryOp::Eq(eq_op) => {
                    let target = if a_ty == Ty::Field(FieldTy::IntI32)
                        && b_ty == Ty::Field(FieldTy::IntI32)
                    {
                        let target = ctx.builder.reserve_stack(1);
                        ctx.builder.code.push(AsmCode::TestInt {
                            a: a_ptr,
                            b: b_ptr,
                            target,
                            bits: IntBits::I32,
                        });
                        target
                    } else if let Ty::Field(FieldTy::RecordId { table_name: a_name }) = &a_ty
                        && let Ty::Field(FieldTy::RecordId { table_name: b_name }) = &b_ty
                    {
                        if a_name != b_name {
                            return Err(())
                        }

                        let target = ctx.builder.reserve_stack(1);
                        ctx.builder.code.push(AsmCode::TestInt {
                            a: a_ptr,
                            b: b_ptr,
                            target,
                            bits: IntBits::U128,
                        });
                        target
                    } else if a_ty == Ty::Field(FieldTy::Text) && b_ty == Ty::Field(FieldTy::Text) {
                        let target = ctx.builder.reserve_stack(1);
                        ctx.builder.code.push(AsmCode::TestString {
                            a: a_ptr,
                            b: b_ptr,
                            target,
                        });
                        target
                    } else {
                        return Err(());
                    };

                    ctx.builder.code.push(AsmCode::SetLiteralConditional {
                        test_result: target,
                        op: crate::asm_code::asm_code::ConditionOp::Eq(*eq_op),
                        target,
                        true_value: true.into(),
                        false_value: Some(false.into()),
                    });
                    (target, Ty::Field(FieldTy::Bool))
                }
            };

            Ok(result)
        }
        Expr::UnaryOp { op, value } => {
            let stack = ctx.builder.stack_pointer();

            let (ptr, ty) = compile_expr_with_ctx(
                &value,
                Ctx {
                    builder: ctx.builder,
                    tables: ctx.tables,
                    is_in_field_access: ctx.is_in_field_access,
                },
            )?;

            ctx.builder.set_stack_pointer(stack);

            let result = match op {
                crate::expr::UnaryOp::Negate => {
                    if ty != Ty::Field(FieldTy::IntI32) {
                        return Err(());
                    }
                    let bits = IntBits::I32;
                    let target = ctx.builder.reserve_stack(bits.bytes());
                    ctx.builder.code.push(AsmCode::NegateNum {
                        value: ptr,
                        target: target,
                        bits,
                    });

                    (target, Ty::Field(FieldTy::IntI32))
                }
                crate::expr::UnaryOp::LogicNot => {
                    if ty != Ty::Field(FieldTy::Bool) {
                        return Err(());
                    }
                    let target = ctx.builder.reserve_stack(1);
                    ctx.builder.code.push(AsmCode::LogicNot {
                        value: ptr,
                        target: target,
                    });

                    (target, Ty::Field(FieldTy::Bool))
                }
            };

            Ok(result)
        }
        Expr::FieldAccess { value, field } => {
            let (ptr, ty) = compile_expr_with_ctx(
                value,
                Ctx {
                    builder: ctx.builder,
                    tables: ctx.tables,
                    is_in_field_access: true,
                },
            )?;

            if let Ty::Table(table) = ty
                && let Some(field) = table.value.field(field)
            {
                let ptr = AsmPointer {
                    namespace: ptr.namespace,
                    offset: ptr.offset + field.offset,
                };
                let result = if ctx.is_in_field_access {
                    (ptr, Ty::Field(field.ty.clone()))
                } else {
                    match &field.ty {
                        FieldTy::IntI32 => {
                            let len = IntBits::I32.bytes();
                            let target = ctx.builder.reserve_stack(len);
                            ctx.builder.code.push(AsmCode::Copy {
                                src: ptr,
                                target,
                                len,
                            });

                            (target, Ty::Field(FieldTy::IntI32))
                        }
                        FieldTy::Bool => {
                            let target = ctx.builder.reserve_stack(1);
                            ctx.builder.code.push(AsmCode::Copy {
                                src: ptr,
                                target,
                                len: 1,
                            });

                            (target, Ty::Field(FieldTy::Bool))
                        }
                        FieldTy::Timestamp => todo!(),
                        FieldTy::Text => {
                            let target = ctx.builder.reserve_stack(AsmSlicePointer::BYTES);
                            ctx.builder.code.push(AsmCode::SetLiteral {
                                target: target,
                                value: ptr.namespace.into(),
                            });
                            ctx.builder.code.push(AsmCode::Copy {
                                src: ptr,
                                target: target.add_offset(4),
                                len: 8,
                            });

                            (target, Ty::Field(FieldTy::Text))
                        }
                        FieldTy::RecordId { table_name } => {
                            let len = IntBits::U128.bytes();
                            let target = ctx.builder.reserve_stack(len);
                            ctx.builder.code.push(AsmCode::Copy {
                                src: ptr,
                                target,
                                len,
                            });

                            (
                                target,
                                Ty::Field(FieldTy::RecordId {
                                    table_name: table_name.clone(),
                                }),
                            )
                        }
                    }
                };

                Ok(result)
            } else {
                return Err(());
            }
        }
        Expr::TableAccess { name } => {
            if !ctx.is_in_field_access {
                return Err(());
            }
            let table = ctx.tables.get(name).unwrap();

            let table_idx = ctx.builder.table_idx(name);

            Ok((
                AsmPointer {
                    namespace: Namespace::Record { idx: table_idx },
                    offset: 0,
                },
                Ty::Table(Named {
                    name: name.clone(),
                    value: table.clone(),
                }),
            ))
        }
        Expr::FnCall { name, args } => todo!(),
    }
}
