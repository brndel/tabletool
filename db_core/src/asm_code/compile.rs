use std::{
    collections::{BTreeMap, HashMap, HashSet},
    ops::ControlFlow,
    sync::Arc,
};

use bytepack::PackFormat;

use crate::{
    asm_code::{
        AsmCompileErr,
        asm_code::{AccessTableIdx, AsmCode, IntBits, Literal},
        asm_iter::AsmIter,
        pointer::{AsmPointer, AsmSlicePointer},
        program::Program,
    },
    defs::table::TableData,
    expr::{BinaryOp, Expr, MathOp},
    named::Named,
    ty::{FieldTy, IterTy, Ty},
    value::FieldValue,
};

use super::pointer::Namespace;

pub fn compile_expr(
    expr: &Expr,
    tables: &BTreeMap<Arc<str>, Arc<TableData>>,
    iter_tables: &HashSet<Arc<str>>,
) -> Result<Program, AsmCompileErr> {
    let mut builder = CodeBuilder::default();
    let ctx = Ctx {
        builder: &mut builder,
        tables,
        iter_tables,
    };

    let (_ptr, ty) = compile_expr_with_ctx(expr, ctx)?;

    Ok(builder.finish(ty))
}

struct Ctx<'a> {
    builder: &'a mut CodeBuilder,
    tables: &'a BTreeMap<Arc<str>, Arc<TableData>>,
    iter_tables: &'a HashSet<Arc<str>>,
}

impl<'a> Ctx<'a> {
    fn nest(&mut self) -> Ctx<'_> {
        Ctx {
            builder: self.builder,
            tables: self.tables,
            iter_tables: self.iter_tables,
        }
    }
}

#[derive(Default)]
struct CodeBuilder {
    const_memory: Vec<u8>,
    code: Vec<AsmCode>,
    record_table_indices: HashMap<Arc<str>, u16>,
    access_table_indices: Vec<Arc<str>>,
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

    fn record_table_idx(&mut self, name: &Arc<str>) -> u16 {
        if let Some(idx) = self.record_table_indices.get(name) {
            *idx
        } else {
            let new_idx = self.record_table_indices.len() as _;
            self.record_table_indices.insert(name.clone(), new_idx);

            new_idx
        }
    }

    fn access_table_idx(&mut self, name: &Arc<str>) -> AccessTableIdx {
        let mut index_iter =
            self.access_table_indices
                .iter()
                .enumerate()
                .filter_map(|(idx, table_name)| {
                    if table_name == name {
                        Some(AccessTableIdx(idx as _))
                    } else {
                        None
                    }
                });

        if let Some(idx) = index_iter.next() {
            idx
        } else {
            let new_idx = AccessTableIdx(self.access_table_indices.len() as _);
            self.access_table_indices.push(name.clone());

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
            record_table_indices: self.record_table_indices,
            access_table_indices: self.access_table_indices,
            return_ty,
        }
    }
}

fn compile_expr_with_ctx(expr: &Expr, mut ctx: Ctx) -> Result<(AsmPointer, Ty), AsmCompileErr> {
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
        Expr::Array(values) => {
            let iter_start_pointer = AsmPointer {
                namespace: Namespace::Stack,
                offset: ctx.builder.stack_pointer,
            }
            .add_offset(AsmIter::BYTES);
            ctx.builder.code.push(AsmCode::Comment("AsmIter"));
            let ptr = ctx.builder.push_stack(AsmIter {
                current_element: iter_start_pointer,
                remaining_elements: values.len() as u32,
            });

            let mut inner_ty: Option<Ty> = None;
            // this value only exists to check wheter the array gets constructed right (every value has an offset of ty_byte_count(ty))
            let mut control_pointer = iter_start_pointer;
            let mut control_ty_offset = 0;

            for expr in values {
                ctx.builder.code.push(AsmCode::Comment("Iter Value"));
                let (ptr, ty) = compile_expr_with_ctx(expr, ctx.nest())?;

                match &inner_ty {
                    Some(inner_ty) => {
                        if inner_ty != &ty {
                            return Err(AsmCompileErr::MissmatchedTy {
                                expected: inner_ty.clone(),
                                found: ty,
                            });
                        }
                    }
                    None => {
                        control_ty_offset = ty_byte_count(&ty);
                        inner_ty = Some(ty);
                    }
                }
                if control_pointer != ptr {
                    println!("invalid array element pointer thingy");
                    panic!();
                }

                control_pointer = control_pointer.add_offset(control_ty_offset);
            }

            let ty = inner_ty.unwrap_or(Ty::Any);

            Ok((
                ptr,
                Ty::Iterator {
                    item_ty: Box::new(ty),
                    kind: IterTy::Array,
                },
            ))
        }
        Expr::BinaryOp { a, op, b } => {
            let stack = ctx.builder.stack_pointer();

            let (a_ptr, a_ty) = compile_expr_with_ctx(&a, ctx.nest())?;

            let (b_ptr, b_ty) = compile_expr_with_ctx(&b, ctx.nest())?;

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
                        return Err(AsmCompileErr::InvalidBinaryOpTy { op: op.clone(), a: a_ty, b: b_ty });
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
                        return Err(AsmCompileErr::InvalidBinaryOpTy { op: op.clone(), a: a_ty, b: b_ty });
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
                        return Err(AsmCompileErr::InvalidBinaryOpTy { op: op.clone(), a: a_ty, b: b_ty });
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
                        return Err(AsmCompileErr::InvalidBinaryOpTy { op: op.clone(), a: a_ty, b: b_ty });
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
                        return Err(AsmCompileErr::InvalidBinaryOpTy { op: op.clone(), a: a_ty, b: b_ty });
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

            let (ptr, ty) = compile_expr_with_ctx(&value, ctx.nest())?;

            ctx.builder.set_stack_pointer(stack);

            let result = match op {
                crate::expr::UnaryOp::Negate => {
                    if ty != Ty::Field(FieldTy::IntI32) {
                        return Err(AsmCompileErr::InvalidUnaryOpTy { op: op.clone(), value: ty });
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
                        return Err(AsmCompileErr::InvalidUnaryOpTy { op: op.clone(), value: ty });
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
            let stack = ctx.builder.stack_pointer;

            let (access, ty) = compile_expr_in_field_access(expr, ctx.nest())?;

            match ty {
                Ty::Field(field_ty) => {
                    let ptr = match access.base {
                        FieldAccessBase::AsmPointer(asm_pointer) => asm_pointer.add_offset(access.offset),
                        FieldAccessBase::RecordAccess { table_idx } => {
                            let target = ctx.builder.reserve_stack(AsmPointer::BYTES);

                            ctx.builder.code.push(AsmCode::GetRecordPointer {
                                table_idx,
                                offset: access.offset,
                                target,
                            });

                            target
                        }
                    };

                    let result = match &field_ty {
                        FieldTy::IntI32 => {
                            ctx.builder.set_stack_pointer(stack);

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
                            ctx.builder.set_stack_pointer(stack);

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
                            ctx.builder.set_stack_pointer(stack);

                            let target = ctx.builder.reserve_stack(AsmSlicePointer::BYTES);
                            ctx.builder.code.push(AsmCode::Copy {
                                src: ptr,
                                target: target.add_offset(4),
                                len: 8,
                            });
                            ctx.builder.code.push(AsmCode::SetLiteral {
                                target: target,
                                value: ptr.namespace.into(),
                            });

                            (target, Ty::Field(FieldTy::Text))
                        }
                        FieldTy::RecordId { table_name } => {
                            ctx.builder.set_stack_pointer(stack);

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
                    };

                    Ok(result)
                }
                Ty::Iterator {
                    item_ty,
                    kind: IterTy::Record,
                } => match item_ty.as_ref() {
                    Ty::Field(FieldTy::IntI32 | FieldTy::Bool) => {
                        ctx.builder.set_stack_pointer(stack);

                        let result = ctx.builder.reserve_stack(AsmIter::BYTES);

                        let FieldAccessBase::RecordAccess { table_idx } = access.base else {
                            return Err(AsmCompileErr::FieldAccessOnNoneRecordIter);
                        };

                        ctx.builder.code.push(AsmCode::GetRecordIterPointer {
                            table_idx: table_idx,
                            offset: access.offset,
                            target: result,
                        });

                        Ok((
                            result,
                            Ty::Iterator {
                                item_ty,
                                kind: IterTy::Record,
                            },
                        ))
                    }
                    ty => Err(AsmCompileErr::FieldAccessOnInvalidTy { ty: Ty::Iterator { item_ty, kind: IterTy::Record }}),
                },
                ty => Err(AsmCompileErr::FieldAccessOnInvalidTy { ty }),
            }
        }
        Expr::RecordFieldAccess { value, field } => {
            let stack = ctx.builder.stack_pointer;

            let (value_ptr, ty) = compile_expr_with_ctx(value, ctx.nest())?;

            match ty {
                Ty::Field(FieldTy::RecordId { table_name }) => {
                    let table_idx = ctx.builder.access_table_idx(&table_name);

                    let table = ctx.tables.get(&table_name).unwrap();

                    let Some(field) = table.field(field) else {
                        return Err(AsmCompileErr::UnkownTableField { field: field.clone(), table_name });
                    };

                    let record_pointer = ctx.builder.reserve_stack(AsmPointer::BYTES);
                    ctx.builder.code.push(AsmCode::QueryRecord {
                        access_table_idx: table_idx,
                        id: value_ptr,
                        offset: field.offset,
                        target: record_pointer,
                    });

                    let ptr = record_pointer;

                    let result = match &field.ty {
                        FieldTy::IntI32 => {
                            ctx.builder.set_stack_pointer(stack);
                            let len = IntBits::I32.bytes();
                            let target = ctx.builder.reserve_stack(len);
                            ctx.builder.code.push(AsmCode::CopyIndirect {
                                indirect_src: ptr,
                                target,
                                len,
                            });

                            (target, Ty::Field(FieldTy::IntI32))
                        }
                        FieldTy::Bool => {
                            ctx.builder.set_stack_pointer(stack);
                            let target = ctx.builder.reserve_stack(1);
                            ctx.builder.code.push(AsmCode::CopyIndirect {
                                indirect_src: ptr,
                                target,
                                len: 1,
                            });

                            (target, Ty::Field(FieldTy::Bool))
                        }
                        FieldTy::Timestamp => todo!(),
                        FieldTy::Text => {
                            let temp_pointer = ctx.builder.reserve_stack(AsmSlicePointer::BYTES);
                            ctx.builder.code.push(AsmCode::Copy {
                                src: ptr,
                                target: temp_pointer,
                                len: 4,
                            });
                            ctx.builder.code.push(AsmCode::CopyIndirect {
                                indirect_src: ptr,
                                target: temp_pointer.add_offset(4),
                                len: 8,
                            });

                            ctx.builder.set_stack_pointer(stack);

                            let target = ctx.builder.reserve_stack(AsmSlicePointer::BYTES);
                            ctx.builder.code.push(AsmCode::Copy {
                                src: temp_pointer,
                                target: target,
                                len: AsmSlicePointer::BYTES,
                            });

                            (target, Ty::Field(FieldTy::Text))
                        }
                        FieldTy::RecordId { table_name } => {
                            ctx.builder.set_stack_pointer(stack);

                            let len = IntBits::U128.bytes();
                            let target = ctx.builder.reserve_stack(len);
                            ctx.builder.code.push(AsmCode::CopyIndirect {
                                indirect_src: ptr,
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
                    };

                    Ok(result)
                }
                ty => Err(AsmCompileErr::FieldAccessOnInvalidTy { ty }),
            }
        }
        Expr::TableAccess { name } => Err(AsmCompileErr::TableAccessWithoutField),
        Expr::FnCall { name, args } => match name.as_ref() {
            "str_len" => {
                if args.len() != 1 {
                    return Err(AsmCompileErr::WrongArgCount { fn_name: "str_len", expected: 1, found: args.len() });
                }

                let arg = &args[0];

                let stack = ctx.builder.stack_pointer;

                let (arg_ptr, arg_ty) = compile_expr_with_ctx(arg, ctx.nest())?;

                match arg_ty {
                    Ty::Field(FieldTy::Text) => {
                        ctx.builder.set_stack_pointer(stack);
                        let result = ctx.builder.reserve_stack(IntBits::U32.bytes());

                        ctx.builder.code.push(AsmCode::Copy {
                            src: arg_ptr.add_offset(AsmSlicePointer::POINTER_LEN_OFFSET),
                            target: result,
                            len: IntBits::U32.bytes(),
                        });

                        Ok((result, Ty::Field(FieldTy::IntI32)))
                    }
                    ty => Err(AsmCompileErr::MissmatchedTy { expected: Ty::Field(FieldTy::Text), found: ty }),
                }
            }
            "sum" => {
                if args.len() != 1 {
                    return Err(AsmCompileErr::WrongArgCount { fn_name: "sum", expected: 1, found: args.len() });
                }

                let arg = &args[0];

                let result_ptr = ctx.builder.push_stack(0_i32);

                let stack = ctx.builder.stack_pointer;

                let (iter_ptr, ty) = compile_expr_with_ctx(arg, ctx.nest())?;

                match ty {
                    Ty::Iterator { item_ty, kind } => match *item_ty {
                        Ty::Field(FieldTy::IntI32) | Ty::Any => {
                            let zero_ptr = ctx.builder.push_stack(0u32);
                            let one_ptr = ctx.builder.push_stack(1u32);
                            let one_u16_ptr = ctx.builder.push_stack(1u16);
                            let element_offset_ptr = ctx.builder.push_stack(IntBits::U32.bytes());

                            let compare_result_ptr =
                                ctx.builder.reserve_stack(IntBits::U32.bytes());
                            let current_value_ptr = ctx.builder.reserve_stack(IntBits::I32.bytes());

                            let loop_check_point = ctx.builder.code.len() + 1;

                            ctx.builder.code.push(AsmCode::TestInt {
                                a: iter_ptr.add_offset(AsmIter::REMAINING_ELEM_OFFSET),
                                b: zero_ptr,
                                target: compare_result_ptr,
                                bits: IntBits::U32,
                            });

                            match kind {
                                IterTy::Array => {
                                    let loop_end_point = ctx.builder.code.len() + 7;

                                    ctx.builder.code.push(AsmCode::JumpConditional {
                                        test_result: compare_result_ptr,
                                        op: super::asm_code::ConditionOp::Eq(crate::expr::EqOp::Eq),
                                        target: loop_end_point,
                                    });

                                    ctx.builder.code.push(AsmCode::CopyIndirect {
                                        indirect_src: iter_ptr
                                            .add_offset(AsmIter::CURRENT_ELEM_PTR_OFFSET),
                                        target: current_value_ptr,
                                        len: IntBits::I32.bytes(),
                                    });
                                    ctx.builder.code.push(AsmCode::MathOp {
                                        a: result_ptr,
                                        b: current_value_ptr,
                                        op: MathOp::Add,
                                        target: result_ptr,
                                        bits: IntBits::I32,
                                    });

                                    let iter_offset_ptr = iter_ptr.add_offset(
                                        AsmIter::CURRENT_ELEM_PTR_OFFSET
                                            + AsmPointer::OFFSET_OFFSET,
                                    );
                                    ctx.builder.code.push(AsmCode::MathOp {
                                        a: iter_offset_ptr,
                                        b: element_offset_ptr,
                                        op: MathOp::Add,
                                        target: iter_offset_ptr,
                                        bits: IntBits::U32,
                                    });

                                    ctx.builder.code.push(AsmCode::MathOp {
                                        a: iter_ptr.add_offset(AsmIter::REMAINING_ELEM_OFFSET),
                                        b: one_ptr,
                                        op: MathOp::Sub,
                                        target: iter_ptr.add_offset(AsmIter::REMAINING_ELEM_OFFSET),
                                        bits: IntBits::U32,
                                    });
                                }
                                IterTy::Record => {
                                    let loop_end_point = ctx.builder.code.len() + 7;

                                    ctx.builder.code.push(AsmCode::JumpConditional {
                                        test_result: compare_result_ptr,
                                        op: super::asm_code::ConditionOp::Eq(crate::expr::EqOp::Eq),
                                        target: loop_end_point,
                                    });

                                    ctx.builder.code.push(AsmCode::CopyIndirect {
                                        indirect_src: iter_ptr
                                            .add_offset(AsmIter::CURRENT_ELEM_PTR_OFFSET),
                                        target: current_value_ptr,
                                        len: IntBits::I32.bytes(),
                                    });
                                    ctx.builder.code.push(AsmCode::MathOp {
                                        a: result_ptr,
                                        b: current_value_ptr,
                                        op: MathOp::Add,
                                        target: result_ptr,
                                        bits: IntBits::I32,
                                    });

                                    let iter_record_idx_ptr = iter_ptr.add_offset(
                                        AsmIter::CURRENT_ELEM_PTR_OFFSET
                                            + AsmPointer::RECORD_IDX_OFFSET,
                                    );
                                    ctx.builder.code.push(AsmCode::MathOp {
                                        a: iter_record_idx_ptr,
                                        b: one_u16_ptr,
                                        op: MathOp::Add,
                                        target: iter_record_idx_ptr,
                                        bits: IntBits::U16,
                                    });

                                    ctx.builder.code.push(AsmCode::MathOp {
                                        a: iter_ptr.add_offset(AsmIter::REMAINING_ELEM_OFFSET),
                                        b: one_ptr,
                                        op: MathOp::Sub,
                                        target: iter_ptr.add_offset(AsmIter::REMAINING_ELEM_OFFSET),
                                        bits: IntBits::U32,
                                    });
                                }
                            }

                            ctx.builder.code.push(AsmCode::Jump {
                                target: loop_check_point,
                            });

                            ctx.builder.set_stack_pointer(stack);

                            Ok((result_ptr, Ty::Field(FieldTy::IntI32)))
                        }
                        ty => return Err(AsmCompileErr::SumWrongIterThiny { ty: ty }),
                    },
                    ty => return Err(AsmCompileErr::SumWrongIterThiny { ty: ty }),
                }
            }
            fn_name => Err(AsmCompileErr::UnkownFn { fn_name: fn_name.to_owned() }),
        },
    }
}

struct FieldAccess {
    offset: u32,
    base: FieldAccessBase,
}

enum FieldAccessBase {
    AsmPointer(AsmPointer),
    RecordAccess { table_idx: AccessTableIdx },
}

impl FieldAccess {
    fn ptr(pointer: AsmPointer) -> Self {
        Self {
            offset: 0,
            base: FieldAccessBase::AsmPointer(pointer),
        }
    }

    fn add_offset(mut self, offset: u32) -> Self {
        self.offset += offset;
        self
    }
}

fn compile_expr_in_field_access(expr: &Expr, mut ctx: Ctx) -> Result<(FieldAccess, Ty), AsmCompileErr> {
    match expr {
        Expr::FieldAccess { value, field } => {
            let (access, ty) = compile_expr_in_field_access(value, ctx.nest())?;

            match ty {
                Ty::Record(table) => {
                    let Some(field) = table.value.field(field) else {
                        return Err(AsmCompileErr::UnkownTableField { field: field.clone(), table_name: table.name });
                    };

                    Ok((access.add_offset(field.offset), Ty::Field(field.ty.clone())))
                }
                Ty::Iterator { item_ty, kind } => match (*item_ty, kind) {
                    (Ty::Record(table), IterTy::Record) => {
                        let Some(field) = table.value.field(field) else {
                            return Err(AsmCompileErr::UnkownTableField { field: field.clone(), table_name: table.name });
                        };

                        Ok((
                            access.add_offset(field.offset),
                            Ty::Iterator {
                                item_ty: Box::new(Ty::Field(field.ty.clone())),
                                kind: IterTy::Record,
                            },
                        ))
                    }
                    _ => Err(AsmCompileErr::FieldAccessOnNoneRecordIter),
                },
                ty => Err(AsmCompileErr::FieldAccessOnInvalidTy { ty }),
            }
        }
        Expr::RecordFieldAccess { value, field } => {
            panic!();
            // let (access, ty) = compile_expr_in_field_access(value, ctx.nest())?;

            // match ty {
            //     Ty::Record(table) => {
            //         let Some(field) = table.value.field(field) else {
            //             return Err(());
            //         };

            //         Ok((access.add_offset(field.offset), Ty::Field(field.ty.clone())))
            //     }
            //     _ => Err(()),
            // }
        }
        Expr::TableAccess { name } => {
            let table = ctx.tables.get(name).unwrap();
            let is_iter_table = ctx.iter_tables.contains(name);

            if is_iter_table {
                let table_idx = ctx.builder.access_table_idx(name);

                Ok((
                    FieldAccess {
                        offset: 0,
                        base: FieldAccessBase::RecordAccess { table_idx },
                    },
                    Ty::Iterator {
                        item_ty: Box::new(Ty::Record(Named {
                            name: name.clone(),
                            value: table.clone(),
                        })),
                        kind: IterTy::Record,
                    },
                ))
            } else {
                let table_idx = ctx.builder.record_table_idx(name);

                Ok((
                    FieldAccess::ptr(AsmPointer {
                        namespace: Namespace::Record { idx: table_idx },
                        offset: 0,
                    }),
                    Ty::Record(Named {
                        name: name.clone(),
                        value: table.clone(),
                    }),
                ))
            }
        }
        expr => {
            let (ptr, ty) = compile_expr_with_ctx(expr, ctx.nest())?;

            Ok((FieldAccess::ptr(ptr), ty))
        }
    }
}

fn ty_byte_count(ty: &Ty) -> u32 {
    match ty {
        Ty::Field(field_ty) => match field_ty {
            FieldTy::IntI32 => IntBits::I32.bytes(),
            FieldTy::Bool => 1,
            FieldTy::Timestamp => todo!(),
            FieldTy::Text => AsmSlicePointer::BYTES,
            FieldTy::RecordId { table_name } => IntBits::U128.bytes(),
        },
        Ty::Record(named) => AsmPointer::BYTES,
        Ty::Iterator { .. } => AsmIter::BYTES,
        Ty::Any => panic!(),
    }
}
