use std::{
    collections::{BTreeMap, HashMap, HashSet},
    ops::ControlFlow,
    sync::Arc,
};

use bytepack::PackFormat;
use chumsky::span::SimpleSpan;

use crate::{
    asm_code::{
        AsmCompileErr, CompletionHint,
        asm_code::{AccessTableIdx, AsmCode, ConditionOp, IntBits, Literal},
        asm_iter::AsmIter,
        complier_diagnostics::CompilerDiagnostics,
        asm_pointer::{AsmPointer, AsmSlicePointer},
        program::Program,
    },
    defs::table::{TableData, TableDef, TableFieldDef},
    expr::{BinaryOp, Expr, MathOp, Spanned},
    named::Named,
    ty::{FieldTy, IterTy, Ty},
    value::FieldValue,
};

use super::asm_pointer::Namespace;

pub fn compile_expr(
    expr: &Expr,
    tables: &BTreeMap<Arc<str>, Arc<TableData>>,
    iter_tables: &HashSet<Arc<str>>,
    diagnostics: &mut CompilerDiagnostics,
) -> Option<Program> {
    let mut builder = CodeBuilder::default();
    let ctx = Ctx {
        builder: &mut builder,
        tables,
        iter_tables,
        diagnostics,
    };

    let (_ptr, ty) = compile_expr_with_ctx(expr, ctx)?;

    Some(builder.finish(ty))
}

struct Ctx<'a> {
    builder: &'a mut CodeBuilder,
    tables: &'a BTreeMap<Arc<str>, Arc<TableData>>,
    iter_tables: &'a HashSet<Arc<str>>,
    diagnostics: &'a mut CompilerDiagnostics,
}

impl<'a> Ctx<'a> {
    pub fn nest(&mut self) -> Ctx<'_> {
        Ctx {
            builder: self.builder,
            tables: self.tables,
            iter_tables: self.iter_tables,
            diagnostics: self.diagnostics,
        }
    }

    pub fn add_table_name_completion(&mut self, span: SimpleSpan) {
        self.diagnostics.add_completion(Spanned::new(
            span,
            CompletionHint {
                options: self
                    .tables
                    .keys()
                    .map(|table_name| table_name.to_string())
                    .collect(),
            },
        ));
    }

    pub fn add_table_fields_completion(&mut self, span: SimpleSpan, table: &TableData) {
        self.diagnostics.add_completion(Spanned::new(
            span,
            CompletionHint {
                options: table.fields().map(|field| field.name.to_string()).collect(),
            },
        ));
    }

    pub fn table(&mut self, span: SimpleSpan, name: &Arc<str>) -> Option<Arc<TableData>> {
        self.add_table_name_completion(span);

        match self.tables.get(name) {
            Some(data) => Some(data.clone()),
            None => {
                self.diagnostics.add_error(
                    span,
                    AsmCompileErr::UnkownTable {
                        table_name: name.clone(),
                    },
                );

                None
            }
        }
    }

    fn record_field(&mut self, span: SimpleSpan, name: &Arc<str>) {}
}

#[derive(Default)]
struct CodeBuilder {
    const_memory: Vec<u8>,
    code: Vec<AsmCode>,
    record_table_indices: HashMap<Arc<str>, u16>,
    access_table_indices: Vec<Arc<str>>,
    stack_pointer: u32,
    max_stack_pointer: u32,
    jump_labels: Vec<usize>,
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
        println!(
            "reserved {} bytes, ptr is now {}",
            byte_count, self.stack_pointer
        );
        self.max_stack_pointer = self.max_stack_pointer.max(self.stack_pointer);

        pointer
    }

    fn set_stack_pointer(&mut self, offset: u32) {
        self.stack_pointer = offset;
        println!("set stack pointer to {}", self.stack_pointer);
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

        for code in &mut self.code {
            match code {
                AsmCode::Jump { target } | AsmCode::JumpConditional { target, .. } => {
                    *target = self.jump_labels.get(*target).cloned().unwrap_or_default();
                }
                _ => (),
            }
        }

        Program {
            const_memory: self.const_memory,
            code: self.code,
            record_table_indices: self.record_table_indices,
            access_table_indices: self.access_table_indices,
            return_ty,
        }
    }

    pub fn reserve_jump_label(&mut self) -> usize {
        let label = self.jump_labels.len();

        self.jump_labels.push(0);

        label
    }

    /// Sets the given jump label to the last added instruction
    pub fn set_jump_label_prev(&mut self, label: usize) {
        self.jump_labels[label] = self.code.len()
    }

    /// Sets the given jump label to the next instruction which will be added
    pub fn set_jump_label_next(&mut self, label: usize) {
        self.jump_labels[label] = self.code.len() + 1
    }
}

fn compile_expr_with_ctx(expr: &Expr, mut ctx: Ctx) -> Option<(AsmPointer, Ty)> {
    match expr {
        Expr::EmptyPlaceholder => None,
        Expr::Literal(field_value) => {
            let result = match &field_value.value {
                FieldValue::Int(value) => {
                    ctx.diagnostics.add_highlight(Spanned::new(
                        field_value.span,
                        super::CompilerHighlight {
                            message: format!("value {value} has type i32"),
                        },
                    ));
                    let ptr = ctx.builder.push_stack(*value);
                    (ptr, Ty::Field(FieldTy::IntI32))
                }
                FieldValue::Bool(value) => {
                    ctx.diagnostics.add_highlight(Spanned::new(
                        field_value.span,
                        super::CompilerHighlight {
                            message: format!("value {value} has type bool"),
                        },
                    ));
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

            Some(result)
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

            for Spanned { value: expr, span } in values {
                ctx.builder.code.push(AsmCode::Comment("Iter Value"));
                let (ptr, ty) = compile_expr_with_ctx(expr, ctx.nest())?;

                match &inner_ty {
                    Some(inner_ty) => {
                        if inner_ty != &ty {
                            ctx.diagnostics.add_error(
                                *span,
                                AsmCompileErr::MissmatchedTy {
                                    expected: inner_ty.clone(),
                                    found: ty,
                                },
                            );
                            return None;
                        }
                    }
                    None => {
                        control_ty_offset = ty_byte_count(&ty);
                        inner_ty = Some(ty);
                    }
                }
                if control_pointer != ptr {
                    panic!(
                        "invalid array element pointer thingy. this is an internal compiler error and should not happen"
                    );
                }

                control_pointer = control_pointer.add_offset(control_ty_offset);
            }

            let ty = inner_ty.unwrap_or(Ty::Any);

            Some((
                ptr,
                Ty::Iterator {
                    item_ty: Box::new(ty),
                    kind: IterTy::Array,
                },
            ))
        }
        Expr::Struct { fields } => {
            let stack = ctx.builder.stack_pointer();

            let fields = fields
                .iter()
                .map(|Spanned { value: field, span }| {
                    let result = compile_expr_with_ctx(&field.value, ctx.nest())?;
                    Some(Spanned {
                        value: Named {
                            name: field.name.clone(),
                            value: result,
                        },
                        span: *span,
                    })
                })
                .collect::<Option<Vec<_>>>()?;

            let table_def = TableDef {
                fields: fields
                    .iter()
                    .map(|Spanned { value: field, span }| {
                        let ty = match &field.value.1 {
                            Ty::Field(field_ty) => field_ty.clone(),
                            ty => {
                                ctx.diagnostics.add_error(
                                    *span,
                                    AsmCompileErr::NonFieldTyInStruct { ty: ty.clone() },
                                );

                                return None;
                            }
                        };

                        Some(Named {
                            name: field.name.clone(),
                            value: TableFieldDef {
                                ty,
                                has_index: false,
                            },
                        })
                    })
                    .collect::<Option<Vec<_>>>()?,
                main_display_field: None,
            };

            let table_data = TableData::from(table_def);

            ctx.builder.set_stack_pointer(stack);
            let ptr = ctx.builder.reserve_stack(table_data.fixed_byte_count());

            Some((ptr, Ty::Struct(Arc::new(table_data))))
        }
        Expr::BinaryOp { a, op, b } => {
            let result = match op.value {
                BinaryOp::Math(math_op) => {
                    let stack = ctx.builder.stack_pointer();

                    let a_result = compile_expr_with_ctx(&a.value, ctx.nest());
                    let b_result = compile_expr_with_ctx(&b.value, ctx.nest());

                    let (a_ptr, a_ty) = a_result?;
                    let (b_ptr, b_ty) = b_result?;

                    ctx.builder.set_stack_pointer(stack);

                    if a_ty == Ty::Field(FieldTy::IntI32) && b_ty == Ty::Field(FieldTy::IntI32) {
                        let bits = IntBits::I32;
                        let target = ctx.builder.reserve_stack(bits.bytes());
                        ctx.builder.code.push(AsmCode::MathOp {
                            a: a_ptr,
                            b: b_ptr,
                            op: math_op,
                            target,
                            bits,
                        });
                        (target, Ty::Field(FieldTy::IntI32))
                    } else {
                        ctx.diagnostics.add_error(
                            op.span,
                            AsmCompileErr::InvalidBinaryOpTy {
                                op: op.value,
                                a: a_ty,
                                b: b_ty,
                            },
                        );
                        return None;
                    }
                }
                BinaryOp::Logic(logic_op) => {
                    let target = ctx.builder.reserve_stack(1);

                    let shortcut_label = ctx.builder.reserve_jump_label();
                    let end_label = ctx.builder.reserve_jump_label();

                    let (shortcut_condition, shortcut_value) = match logic_op {
                        crate::expr::LogicOp::And => (ConditionOp::BoolFalse, false),
                        crate::expr::LogicOp::Or => (ConditionOp::BoolTrue, true),
                    };

                    let fallthrough_value = !shortcut_value;

                    let stack = ctx.builder.stack_pointer();

                    let a_result = compile_expr_with_ctx(&a.value, ctx.nest());

                    if let Some((a_ptr, _)) = a_result {
                        ctx.builder.code.push(AsmCode::JumpConditional {
                            test_result: a_ptr,
                            op: shortcut_condition,
                            target: shortcut_label,
                        });
                    }

                    ctx.builder.set_stack_pointer(stack);
                    let b_result = compile_expr_with_ctx(&b.value, ctx.nest());

                    if let Some((b_ptr, _)) = b_result {
                        ctx.builder.code.push(AsmCode::JumpConditional {
                            test_result: b_ptr,
                            op: shortcut_condition,
                            target: shortcut_label,
                        });
                    }

                    let (_, a_ty) = a_result?;
                    let (_, b_ty) = b_result?;

                    ctx.builder.set_stack_pointer(stack);

                    if a_ty == Ty::Field(FieldTy::Bool) && b_ty == Ty::Field(FieldTy::Bool) {
                        ctx.builder.code.push(AsmCode::SetLiteral {
                            target,
                            value: fallthrough_value.into(),
                        });
                        ctx.builder.code.push(AsmCode::Jump { target: end_label });

                        ctx.builder.set_jump_label_next(shortcut_label);
                        ctx.builder.code.push(AsmCode::SetLiteral {
                            target,
                            value: shortcut_value.into(),
                        });

                        ctx.builder.set_jump_label_next(end_label);

                        (target, Ty::Field(FieldTy::Bool))
                    } else {
                        ctx.diagnostics.add_error(
                            op.span,
                            AsmCompileErr::InvalidBinaryOpTy {
                                op: op.value,
                                a: a_ty,
                                b: b_ty,
                            },
                        );
                        return None;
                    }
                }
                BinaryOp::Compare(compare_op) => {
                    let stack = ctx.builder.stack_pointer();

                    let a_result = compile_expr_with_ctx(&a.value, ctx.nest());
                    let b_result = compile_expr_with_ctx(&b.value, ctx.nest());

                    let (a_ptr, a_ty) = a_result?;
                    let (b_ptr, b_ty) = b_result?;

                    ctx.builder.set_stack_pointer(stack);

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
                        ctx.diagnostics.add_error(
                            op.span,
                            AsmCompileErr::InvalidBinaryOpTy {
                                op: op.value,
                                a: a_ty,
                                b: b_ty,
                            },
                        );
                        return None;
                    };

                    ctx.builder.code.push(AsmCode::SetLiteralConditional {
                        test_result: target,
                        op: crate::asm_code::asm_code::ConditionOp::Compare(compare_op),
                        target,
                        true_value: true.into(),
                        false_value: Some(false.into()),
                    });
                    (target, Ty::Field(FieldTy::Bool))
                }
                BinaryOp::Eq(eq_op) => {
                    let stack = ctx.builder.stack_pointer();

                    let a_result = compile_expr_with_ctx(&a.value, ctx.nest());
                    let b_result = compile_expr_with_ctx(&b.value, ctx.nest());

                    let (a_ptr, a_ty) = a_result?;
                    let (b_ptr, b_ty) = b_result?;

                    ctx.builder.set_stack_pointer(stack);

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
                            ctx.diagnostics.add_error(
                                op.span,
                                AsmCompileErr::InvalidBinaryOpTy {
                                    op: op.value,
                                    a: a_ty,
                                    b: b_ty,
                                },
                            );
                            return None;
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
                        ctx.diagnostics.add_error(
                            op.span,
                            AsmCompileErr::InvalidBinaryOpTy {
                                op: op.value,
                                a: a_ty,
                                b: b_ty,
                            },
                        );
                        return None;
                    };

                    ctx.builder.code.push(AsmCode::SetLiteralConditional {
                        test_result: target,
                        op: crate::asm_code::asm_code::ConditionOp::Eq(eq_op),
                        target,
                        true_value: true.into(),
                        false_value: Some(false.into()),
                    });
                    (target, Ty::Field(FieldTy::Bool))
                }
            };

            Some(result)
        }
        Expr::UnaryOp { op, value } => {
            let stack = ctx.builder.stack_pointer();

            let (ptr, ty) = compile_expr_with_ctx(&value.value, ctx.nest())?;

            ctx.builder.set_stack_pointer(stack);

            let result = match op.value {
                crate::expr::UnaryOp::Negate => {
                    if ty != Ty::Field(FieldTy::IntI32) {
                        ctx.diagnostics.add_error(
                            op.span,
                            AsmCompileErr::InvalidUnaryOpTy {
                                op: op.value,
                                value: ty,
                            },
                        );
                        return None;
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
                        ctx.diagnostics.add_error(
                            op.span,
                            AsmCompileErr::InvalidUnaryOpTy {
                                op: op.value,
                                value: ty,
                            },
                        );
                        return None;
                    }
                    let target = ctx.builder.reserve_stack(1);
                    ctx.builder.code.push(AsmCode::LogicNot {
                        value: ptr,
                        target: target,
                    });

                    (target, Ty::Field(FieldTy::Bool))
                }
            };

            Some(result)
        }
        Expr::FieldAccess {
            value: Spanned { value: _, span },
            ..
        } => {
            let stack = ctx.builder.stack_pointer;

            let FieldAccessResult {
                access,
                ty,
                is_record_access,
            } = compile_expr_in_field_access(expr, ctx.nest())?;

            match ty {
                Ty::Field(field_ty) => {
                    let ptr = access.to_ptr(ctx.builder);

                    let result = match &field_ty {
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
                        FieldTy::Timestamp => {
                            ctx.builder.set_stack_pointer(stack);

                            let len = IntBits::I64.bytes();
                            let target = ctx.builder.reserve_stack(len);
                            ctx.builder.code.push(AsmCode::CopyIndirect {
                                indirect_src: ptr,
                                target,
                                len,
                            });

                            (target, Ty::Field(FieldTy::Timestamp))
                        }
                        FieldTy::Text => {
                            ctx.builder.set_stack_pointer(stack);

                            let target = ctx.builder.reserve_stack(AsmSlicePointer::BYTES);

                            if is_record_access {
                                let temp_ptr = ctx.builder.reserve_stack(AsmPointer::BYTES);
                                ctx.builder.code.push(AsmCode::Copy {
                                    src: ptr,
                                    target: temp_ptr,
                                    len: AsmPointer::BYTES,
                                });

                                ctx.builder.code.push(AsmCode::CopyIndirect {
                                    indirect_src: temp_ptr,
                                    target: target.add_offset(4),
                                    len: 8,
                                });
                                ctx.builder.code.push(AsmCode::Copy {
                                    src: temp_ptr,
                                    target: target,
                                    len: 4,
                                });
                            } else {
                                ctx.builder.code.push(AsmCode::CopyIndirect {
                                    indirect_src: ptr,
                                    target: target,
                                    len: AsmSlicePointer::BYTES,
                                });
                            }

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

                    Some(result)
                }
                Ty::Iterator {
                    item_ty,
                    kind: IterTy::Record,
                } => match item_ty.as_ref() {
                    Ty::Field(FieldTy::IntI32 | FieldTy::Bool) => {
                        ctx.builder.set_stack_pointer(stack);

                        let result = ctx.builder.reserve_stack(AsmIter::BYTES);

                        let FieldAccessBase::RecordAccess { table_idx } = access.base else {
                            todo!();
                            // return Err(AsmCompileErr::FieldAccessOnNoneRecordIter);
                        };

                        ctx.builder.code.push(AsmCode::GetRecordIterPointer {
                            table_idx: table_idx,
                            offset: access.offset,
                            target: result,
                        });

                        Some((
                            result,
                            Ty::Iterator {
                                item_ty,
                                kind: IterTy::Record,
                            },
                        ))
                    }
                    _ => {
                        ctx.diagnostics.add_error(
                            *span,
                            AsmCompileErr::FieldAccessOnInvalidTy {
                                ty: Ty::Iterator {
                                    item_ty,
                                    kind: IterTy::Record,
                                },
                            },
                        );

                        return None;
                    }
                },
                ty => {
                    ctx.diagnostics
                        .add_error(*span, AsmCompileErr::FieldAccessOnInvalidTy { ty });

                    return None;
                }
            }
        }
        Expr::Variable {
            name: Spanned { value: _, span },
        } => {
            ctx.add_table_name_completion(*span);
            ctx.diagnostics
                .add_error(*span, AsmCompileErr::TableAccessWithoutField);

            return None;
        }
        Expr::FnCall { name, args } => match name.value.as_ref() {
            "str_len" => {
                if args.len() != 1 {
                    ctx.diagnostics.add_error(
                        name.span,
                        AsmCompileErr::WrongArgCount {
                            fn_name: "str_len",
                            expected: 1,
                            found: args.len(),
                        },
                    );
                    return None;
                }

                let arg = &args[0];

                let stack = ctx.builder.stack_pointer;

                let (arg_ptr, arg_ty) = compile_expr_with_ctx(&arg.value, ctx.nest())?;

                match arg_ty {
                    Ty::Field(FieldTy::Text) => {
                        ctx.builder.set_stack_pointer(stack);
                        let result = ctx.builder.reserve_stack(IntBits::U32.bytes());

                        ctx.builder.code.push(AsmCode::Copy {
                            src: arg_ptr.add_offset(AsmSlicePointer::POINTER_LEN_OFFSET),
                            target: result,
                            len: IntBits::U32.bytes(),
                        });

                        Some((result, Ty::Field(FieldTy::IntI32)))
                    }
                    ty => {
                        ctx.diagnostics.add_error(
                            arg.span,
                            AsmCompileErr::MissmatchedTy {
                                expected: Ty::Field(FieldTy::Text),
                                found: ty,
                            },
                        );
                        return None;
                    }
                }
            }
            "sum" => {
                if args.len() != 1 {
                    ctx.diagnostics.add_error(
                        name.span,
                        AsmCompileErr::WrongArgCount {
                            fn_name: "sum",
                            expected: 1,
                            found: args.len(),
                        },
                    );
                    return None;
                }

                let arg = &args[0];

                let result_ptr = ctx.builder.push_stack(0_i32);

                let stack = ctx.builder.stack_pointer;

                let (iter_ptr, ty) = compile_expr_with_ctx(&arg.value, ctx.nest())?;

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

                            let loop_check_label = ctx.builder.reserve_jump_label();
                            let loop_end_label = ctx.builder.reserve_jump_label();

                            ctx.builder.set_jump_label_next(loop_check_label);

                            ctx.builder.code.push(AsmCode::TestInt {
                                a: iter_ptr.add_offset(AsmIter::REMAINING_ELEM_OFFSET),
                                b: zero_ptr,
                                target: compare_result_ptr,
                                bits: IntBits::U32,
                            });

                            ctx.builder.code.push(AsmCode::JumpConditional {
                                test_result: compare_result_ptr,
                                op: super::asm_code::ConditionOp::Eq(crate::expr::EqOp::Eq),
                                target: loop_end_label,
                            });

                            ctx.builder.code.push(AsmCode::CopyIndirect {
                                indirect_src: iter_ptr.add_offset(AsmIter::CURRENT_ELEM_PTR_OFFSET),
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

                            match kind {
                                IterTy::Array => {
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
                                }
                                IterTy::Record => {
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
                                }
                            }

                            ctx.builder.code.push(AsmCode::MathOp {
                                a: iter_ptr.add_offset(AsmIter::REMAINING_ELEM_OFFSET),
                                b: one_ptr,
                                op: MathOp::Sub,
                                target: iter_ptr.add_offset(AsmIter::REMAINING_ELEM_OFFSET),
                                bits: IntBits::U32,
                            });

                            ctx.builder.code.push(AsmCode::Jump {
                                target: loop_check_label,
                            });

                            ctx.builder.set_jump_label_next(loop_end_label);

                            ctx.builder.set_stack_pointer(stack);

                            Some((result_ptr, Ty::Field(FieldTy::IntI32)))
                        }
                        ty => {
                            ctx.diagnostics.add_error(
                                arg.span,
                                AsmCompileErr::SumNotCalledOnI32Iter { ty: ty },
                            );
                            return None;
                        }
                    },
                    ty => {
                        ctx.diagnostics
                            .add_error(arg.span, AsmCompileErr::SumNotCalledOnI32Iter { ty: ty });
                        return None;
                    }
                }
            }
            fn_name => {
                ctx.diagnostics.add_error(
                    name.span,
                    AsmCompileErr::UnkownFn {
                        fn_name: fn_name.to_owned(),
                    },
                );
                return None;
            }
        },
        Expr::LambdaFn { args, body } => {
            todo!()
        }
    }
}

struct FieldAccess {
    offset: u32,
    base: FieldAccessBase,
}

enum FieldAccessBase {
    AsmPointer(AsmPointer),
    RecordAccess {
        table_idx: AccessTableIdx,
    },
    QueryRecord {
        table_idx: AccessTableIdx,
        id: AsmPointer,
    },
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

    fn to_ptr(&self, builder: &mut CodeBuilder) -> AsmPointer {
        match self.base {
            FieldAccessBase::AsmPointer(asm_pointer) => {
                let target = builder.push_stack(asm_pointer.add_offset(self.offset));
                target
            }
            FieldAccessBase::RecordAccess { table_idx } => {
                let target = builder.reserve_stack(AsmPointer::BYTES);

                builder.code.push(AsmCode::GetRecordPointer {
                    table_idx,
                    offset: self.offset,
                    target,
                });

                target
            }
            FieldAccessBase::QueryRecord { table_idx, id } => {
                let target = builder.reserve_stack(AsmPointer::BYTES);

                builder.code.push(AsmCode::QueryRecordIndirect {
                    access_table_idx: table_idx,
                    indirect_id: id,
                    offset: self.offset,
                    target,
                });

                target
            }
        }
    }
}

struct FieldAccessResult {
    access: FieldAccess,
    ty: Ty,
    is_record_access: bool,
}

fn compile_expr_in_field_access(expr: &Expr, mut ctx: Ctx) -> Option<FieldAccessResult> {
    match expr {
        Expr::FieldAccess {
            value,
            dot_span,
            field: None,
        } => {
            let FieldAccessResult {
                access,
                ty,
                is_record_access: _,
            } = compile_expr_in_field_access(&value.value, ctx.nest())?;

            if let Ty::Record(table) = ty {
                ctx.add_table_fields_completion(*dot_span, &table.value);
            }

            ctx.diagnostics
                .add_error(*dot_span, AsmCompileErr::FieldAccessMissingName);
            return None;
        }
        Expr::FieldAccess {
            value,
            field:
                Some(Spanned {
                    value: field,
                    span: field_span,
                }),
            dot_span,
        } => {
            let FieldAccessResult {
                access,
                ty,
                is_record_access: _,
            } = compile_expr_in_field_access(&value.value, ctx.nest())?;

            match ty {
                Ty::Record(table) => {
                    ctx.add_table_fields_completion(*field_span, &table.value);

                    let Some(field) = table.value.field(field) else {
                        ctx.diagnostics.add_error(
                            *field_span,
                            AsmCompileErr::UnkownTableField {
                                field: field.clone(),
                                table_name: table.name,
                            },
                        );
                        return None;
                    };

                    Some(FieldAccessResult {
                        access: access.add_offset(field.offset),
                        ty: Ty::Field(field.ty.clone()),
                        is_record_access: true,
                    })
                }
                Ty::Struct(table) => {
                    let Some(field) = table.field(field) else {
                        ctx.diagnostics.add_error(
                            *field_span,
                            AsmCompileErr::UnkownTableField {
                                field: field.clone(),
                                table_name: "struct table".into(),
                            },
                        );
                        return None;
                    };

                    Some(FieldAccessResult {
                        access: access.add_offset(field.offset),
                        ty: Ty::Field(field.ty.clone()),
                        is_record_access: false,
                    })
                }
                Ty::Iterator { item_ty, kind } => match (*item_ty, kind) {
                    (Ty::Record(table), IterTy::Record) => {
                        let Some(field) = table.value.field(field) else {
                            ctx.diagnostics.add_error(
                                *field_span,
                                AsmCompileErr::UnkownTableField {
                                    field: field.clone(),
                                    table_name: table.name,
                                },
                            );
                            return None;
                        };

                        Some(FieldAccessResult {
                            access: access.add_offset(field.offset),
                            ty: Ty::Iterator {
                                item_ty: Box::new(Ty::Field(field.ty.clone())),
                                kind: IterTy::Record,
                            },
                            is_record_access: true,
                        })
                    }
                    _ => {
                        ctx.diagnostics
                            .add_error(*field_span, AsmCompileErr::FieldAccessOnNoneRecordIter);
                        return None;
                    }
                },
                Ty::Field(FieldTy::RecordId { table_name }) => {
                    let table_idx = ctx.builder.access_table_idx(&table_name);

                    let table = ctx.tables.get(&table_name).unwrap();

                    let Some(field) = table.field(field) else {
                        ctx.diagnostics.add_error(
                            *field_span,
                            AsmCompileErr::UnkownTableField {
                                field: field.clone(),
                                table_name: table_name,
                            },
                        );
                        return None;
                    };

                    let ptr = access.to_ptr(ctx.builder);

                    Some(FieldAccessResult {
                        access: FieldAccess {
                            offset: field.offset,
                            base: FieldAccessBase::QueryRecord { table_idx, id: ptr },
                        },
                        ty: Ty::Field(field.ty.clone()),
                        is_record_access: true,
                    })
                }
                ty => {
                    ctx.diagnostics
                        .add_error(*field_span, AsmCompileErr::FieldAccessOnInvalidTy { ty });
                    return None;
                }
            }
        }
        Expr::Variable {
            name: Spanned { value: name, span },
        } => {
            let table = ctx.table(*span, name)?;

            let is_iter_table = ctx.iter_tables.contains(name);

            if is_iter_table {
                let table_idx = ctx.builder.access_table_idx(name);

                Some(FieldAccessResult {
                    access: FieldAccess {
                        offset: 0,
                        base: FieldAccessBase::RecordAccess { table_idx },
                    },
                    ty: Ty::Iterator {
                        item_ty: Box::new(Ty::Record(Named {
                            name: name.clone(),
                            value: table,
                        })),
                        kind: IterTy::Record,
                    },
                    is_record_access: true,
                })
            } else {
                let table_idx = ctx.builder.record_table_idx(name);

                Some(FieldAccessResult {
                    access: FieldAccess::ptr(AsmPointer {
                        namespace: Namespace::Record { idx: table_idx },
                        offset: 0,
                    }),
                    ty: Ty::Record(Named {
                        name: name.clone(),
                        value: table,
                    }),
                    is_record_access: true,
                })
            }
        }
        expr => {
            let (ptr, ty) = compile_expr_with_ctx(expr, ctx.nest())?;

            Some(FieldAccessResult {
                access: FieldAccess::ptr(ptr),
                ty,
                is_record_access: false,
            })
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
        Ty::Struct(table) => todo!(),
        Ty::Iterator { .. } => AsmIter::BYTES,
        Ty::Any => panic!(),
    }
}
