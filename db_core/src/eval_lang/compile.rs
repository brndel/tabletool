use std::{
    collections::{BTreeMap, HashMap},
    mem,
    sync::Arc,
};

use bytepack::PackFormat;
use chumsky::span::SimpleSpan;
use ulid::Ulid;
use wasm_encoder::{Function, Instruction, InstructionSink, MemArg, ValType};

use crate::{
    compile::{CompileErr, CompilerDiagnostics, CompletionHint},
    defs::table::TableData,
    eval_lang::{
        builder::{CodeBuilder, ConstMemoryBuilder, FunctionBuilder, FunctionRegistry},
        program::Program,
    },
    expr::{
        BinaryOp, CompareOp, EqOp, Expr, ExprBlock, LogicOp, MathOp, QueryExpr, Spanned, UnaryOp,
    },
    ty::{FieldTy, IterKind, Ty},
    value::FieldValue,
};

pub fn compile(
    expr: &Expr,
    diagnostics: &mut CompilerDiagnostics,
    tables: &BTreeMap<Arc<str>, Arc<TableData>>,
) -> Option<Program> {
    let mut builder = CodeBuilder {
        const_memory: ConstMemoryBuilder::new(),
        functions: FunctionRegistry::new(),
        table_indices: Vec::new(),
    };

    let mut main_fn = FunctionBuilder::new(vec![]);

    let mut ctx = Ctx {
        global: GlobalCtx {
            builder: &mut builder,
            diagnostics,
            tables,
        },
        local: LocalCtx {
            func_builder: &mut main_fn,
            scopes: &mut Vec::new(),
        },
        scope: 0,
    };

    let return_ty = compile_expr(expr, &mut ctx)?;

    main_fn.instr(&Instruction::End);
    main_fn.set_export_name(CodeBuilder::MAIN_FN_NAME.into());
    main_fn.set_return_type(return_ty);

    builder.functions.push(main_fn.finish()?);

    Some(builder.finish())
}

pub struct GlobalCtx<'a> {
    builder: &'a mut CodeBuilder,
    // const_builder: &'a mut ConstMemoryBuilder,
    diagnostics: &'a mut CompilerDiagnostics,
    // table_indices: &'a mut Vec<Arc<str>>,
    tables: &'a BTreeMap<Arc<str>, Arc<TableData>>,
}

struct VariableDef {
    span: SimpleSpan,
    local: u32,
    ty: Ty,
}

pub struct LocalCtx<'a> {
    func_builder: &'a mut FunctionBuilder,
    scopes: &'a mut Vec<HashMap<Arc<str>, VariableDef>>,
}

pub struct Ctx<'a> {
    global: GlobalCtx<'a>,
    local: LocalCtx<'a>,
    scope: usize,
}

impl<'a> Ctx<'a> {
    pub fn instr(&mut self, instr: &Instruction) -> &mut Self {
        self.local.func_builder.instr(instr);

        self
    }

    pub fn instr_sink(&mut self) -> InstructionSink<'_> {
        self.local.func_builder.instr_sink()
    }

    pub fn table_index(&mut self, table_name: &Arc<str>) -> u32 {
        if let Some((idx, _)) = self
            .global
            .builder
            .table_indices
            .iter()
            .enumerate()
            .filter(|(_, name)| table_name.as_ref() == name.as_ref())
            .next()
        {
            idx as u32
        } else {
            let idx = self.global.builder.table_indices.len();
            self.global.builder.table_indices.push(table_name.clone());

            idx as u32
        }
    }

    pub fn block(&mut self) -> Ctx<'_> {
        let new_scope = self.scope + 1;

        self.local
            .scopes
            .resize_with(new_scope + 1, Default::default);
        self.local.scopes[new_scope].clear();

        Ctx {
            global: GlobalCtx {
                builder: self.global.builder,
                diagnostics: self.global.diagnostics,
                tables: self.global.tables,
            },
            local: LocalCtx {
                func_builder: self.local.func_builder,
                scopes: self.local.scopes,
            },
            scope: self.scope + 1,
        }
    }

    pub fn get_variable(&self, name: &str) -> Option<&VariableDef> {
        for i in (0..=self.scope).rev() {
            let scope = &self.local.scopes[i];

            if let Some(var) = scope.get(name) {
                return Some(var);
            }
        }

        None
    }

    pub fn set_variable(&mut self, name: &Arc<str>, variable: VariableDef) {
        let scope = &mut self.local.scopes[self.scope];

        scope.insert(name.clone(), variable);
    }
}

pub fn compile_expr(expr: &Expr, ctx: &mut Ctx) -> Option<Ty> {
    match expr {
        Expr::Literal(Spanned { value, span }) => {
            let ty = match value {
                FieldValue::Int(value) => {
                    ctx.instr(&Instruction::I32Const(*value));
                    FieldTy::IntI32
                }
                FieldValue::Bool(value) => {
                    ctx.instr(&Instruction::I32Const(*value as _));
                    FieldTy::Bool
                }
                FieldValue::Timestamp(value) => {
                    ctx.instr(&Instruction::I64Const(value.timestamp()));
                    FieldTy::Timestamp
                }
                FieldValue::Text(value) => {
                    let (addr, len) = ctx
                        .global
                        .builder
                        .const_memory
                        .alloc_const(value.as_bytes());

                    ctx.instr(&Instruction::I32Const(addr))
                        .instr(&Instruction::I32Const(len));
                    FieldTy::Text
                }
                FieldValue::RecordId { id, table_name } => {
                    ctx.instr(&Instruction::V128Const(id.0.cast_signed()));

                    FieldTy::RecordId {
                        table_name: table_name.clone(),
                    }
                }
            };

            Some(Ty::Field(ty))
        }
        Expr::Array(values) => {
            let mut item_ty = None;

            let mut item_mem_size: u32 = 0;
            let mut mem_address_local = ctx.local.func_builder.local(ValType::I32);

            for (idx, Spanned { value, span }) in values.iter().enumerate() {
                let ty = compile_expr(value, ctx)?;

                match &item_ty {
                    Some(item_ty) => {
                        if &ty != item_ty {
                            ctx.global.diagnostics.add_error(
                                *span,
                                CompileErr::MissmatchedTy {
                                    expected: item_ty.clone(),
                                    found: ty,
                                },
                            );
                            return None;
                        }
                    }
                    None => {
                        item_mem_size = mem_size_of_ty(&ty);
                        item_ty = Some(ty);

                        let alloc_fn_idx = ctx.global.builder.functions.get("alloc").unwrap();

                        let total_bytes_needed = item_mem_size * values.len() as u32;
                        ctx.instr_sink()
                            .i32_const(total_bytes_needed.cast_signed())
                            .call(alloc_fn_idx)
                            .local_set(mem_address_local);
                    }
                }

                let item_ty = item_ty.as_ref().unwrap();

                let offset = (item_mem_size * idx as u32) as u64;
                let mem_arg = MemArg {
                    offset,
                    align: 0,
                    memory_index: CodeBuilder::MEMORY_INDEX,
                };

                let temp_local = match item_ty {
                    Ty::Field(field_ty) => match field_ty {
                        FieldTy::IntI32 => ctx.local.func_builder.i32_temp_local(),
                        FieldTy::Bool => ctx.local.func_builder.i32_temp_local(),
                        FieldTy::Timestamp => ctx.local.func_builder.i64_temp_local(),
                        FieldTy::Text => ctx.local.func_builder.i32_temp_local(),
                        FieldTy::RecordId { table_name } => {
                            ctx.local.func_builder.v128_temp_local()
                        }
                    },
                    Ty::Record(named) => todo!(),
                    Ty::Struct(table_data) => todo!(),
                    Ty::Iterator { item_ty, kind } => todo!(),
                    Ty::Unit => todo!(),
                    Ty::Any => todo!(),
                };

                let mut instr = ctx.instr_sink();
                let instr = instr
                    .local_set(temp_local)
                    .local_get(mem_address_local)
                    .local_get(temp_local);

                match item_ty {
                    Ty::Field(field_ty) => match field_ty {
                        FieldTy::IntI32 => instr.i32_store(mem_arg),
                        FieldTy::Bool => instr.i32_store8(mem_arg),
                        FieldTy::Timestamp => instr.i64_store(mem_arg),
                        FieldTy::Text => {
                            let len_mem_arg = MemArg {
                                offset: offset + 4,
                                align: 0,
                                memory_index: CodeBuilder::MEMORY_INDEX,
                            };
                            instr
                                .i32_store(len_mem_arg)
                                .local_set(temp_local)
                                .local_get(mem_address_local)
                                .local_get(temp_local)
                                .i32_store(mem_arg)
                        }
                        FieldTy::RecordId { .. } => instr.v128_store(mem_arg),
                    },
                    Ty::Record(named) => todo!(),
                    Ty::Struct(table_data) => todo!(),
                    Ty::Iterator { item_ty, kind } => todo!(),
                    Ty::Unit => todo!(),
                    Ty::Any => todo!(),
                };
            }

            ctx.instr_sink()
                .local_get(mem_address_local)
                .i32_const((values.len() as u32).cast_signed());

            let item_ty = item_ty.unwrap_or(Ty::Any);

            Some(Ty::Iterator {
                item_ty: Box::new(item_ty),
                kind: IterKind::Array,
            })
        }
        Expr::Struct { fields } => todo!(),
        Expr::BinaryOp {
            a,
            op: Spanned {
                value: op,
                span: op_span,
            },
            b,
        } => {
            let Spanned {
                value: a,
                span: a_span,
            } = a.as_ref();
            let Spanned {
                value: b,
                span: b_span,
            } = b.as_ref();

            let a = compile_expr(a, ctx);
            let b = compile_expr(b, ctx);

            let (a, b) = (a?, b?);

            match op {
                BinaryOp::Math(math_op) => {
                    let is_right_type =
                        a == Ty::Field(FieldTy::IntI32) && b == Ty::Field(FieldTy::IntI32);

                    if !is_right_type {
                        ctx.global
                            .diagnostics
                            .add_error(*op_span, CompileErr::InvalidBinaryOpTy { op: *op, a, b });
                        return None;
                    }

                    match math_op {
                        MathOp::Add => {
                            ctx.instr(&Instruction::I32Add);
                        }
                        MathOp::Sub => {
                            ctx.instr(&Instruction::I32Sub);
                        }
                        MathOp::Mul => {
                            ctx.instr(&Instruction::I32Mul);
                        }
                        MathOp::Div => {
                            ctx.instr(&Instruction::I32DivS);
                        }
                    }

                    Some(Ty::Field(FieldTy::IntI32))
                }
                BinaryOp::Logic(logic_op) => {
                    let is_right_type =
                        a == Ty::Field(FieldTy::Bool) && b == Ty::Field(FieldTy::Bool);

                    if !is_right_type {
                        ctx.global
                            .diagnostics
                            .add_error(*op_span, CompileErr::InvalidBinaryOpTy { op: *op, a, b });
                        return None;
                    }

                    match logic_op {
                        LogicOp::And => {
                            ctx.instr(&Instruction::I32And);
                        }
                        LogicOp::Or => {
                            ctx.instr(&Instruction::I32Or);
                        }
                    }

                    Some(Ty::Field(FieldTy::Bool))
                }
                BinaryOp::Compare(compare_op) => {
                    let is_right_type =
                        a == Ty::Field(FieldTy::IntI32) && b == Ty::Field(FieldTy::IntI32);

                    if !is_right_type {
                        ctx.global
                            .diagnostics
                            .add_error(*op_span, CompileErr::InvalidBinaryOpTy { op: *op, a, b });
                        return None;
                    }

                    match compare_op {
                        CompareOp::Less => {
                            ctx.instr(&Instruction::I32LtS);
                        }
                        CompareOp::LessEq => {
                            ctx.instr(&Instruction::I32LeS);
                        }
                        CompareOp::Greater => {
                            ctx.instr(&Instruction::I32GtS);
                        }
                        CompareOp::GreaterEq => {
                            ctx.instr(&Instruction::I32GeS);
                        }
                    }

                    Some(Ty::Field(FieldTy::Bool))
                }
                BinaryOp::Eq(eq_op) => {
                    match (a, b) {
                        (Ty::Field(FieldTy::IntI32), Ty::Field(FieldTy::IntI32))
                        | (Ty::Field(FieldTy::Bool), Ty::Field(FieldTy::Bool)) => match eq_op {
                            EqOp::Eq => {
                                ctx.instr(&Instruction::I32Eq);
                            }
                            EqOp::Neq => {
                                ctx.instr(&Instruction::I32Ne);
                            }
                        },
                        (a, b) => {
                            ctx.global.diagnostics.add_error(
                                *op_span,
                                CompileErr::InvalidBinaryOpTy { op: *op, a, b },
                            );
                            return None;
                        }
                    }

                    Some(Ty::Field(FieldTy::Bool))
                }
            }
        }
        Expr::UnaryOp {
            op: Spanned {
                value: op,
                span: op_span,
            },
            value,
        } => {
            let Spanned {
                value,
                span: value_span,
            } = value.as_ref();

            let ty = compile_expr(value, ctx)?;

            match (op, ty) {
                (UnaryOp::Negate, Ty::Field(FieldTy::IntI32)) => {
                    let local = ctx.local.func_builder.i32_temp_local();

                    ctx.instr(&Instruction::LocalSet(local))
                        .instr(&Instruction::I32Const(0))
                        .instr(&Instruction::LocalGet(local))
                        .instr(&Instruction::I32Sub);

                    Some(Ty::Field(FieldTy::IntI32))
                }
                (UnaryOp::LogicNot, Ty::Field(FieldTy::Bool)) => {
                    ctx.instr(&Instruction::I32Eqz);

                    Some(Ty::Field(FieldTy::Bool))
                }
                (op, ty) => {
                    ctx.global.diagnostics.add_error(
                        *op_span,
                        CompileErr::InvalidUnaryOpTy { op: *op, value: ty },
                    );
                    None
                }
            }
        }
        Expr::FieldAccess { .. } => {
            let access = compile_field_access(expr, ctx)?;
            let ty = access.ty();

            access.get(ctx.local.func_builder);

            Some(ty.clone())
        }
        Expr::Variable { name } => {
            let Some(variable) = ctx.get_variable(&name.value) else {
                ctx.global.diagnostics.add_error(
                    name.span,
                    CompileErr::UnkownVar {
                        var: name.value.clone(),
                    },
                );
                return None;
            };

            let local = variable.local;
            let ty = variable.ty.clone();
            let mut sink = ctx.instr_sink();

            match &ty {
                Ty::Field(field_ty) => match field_ty {
                    FieldTy::IntI32 => sink.local_get(local),
                    FieldTy::Bool => sink.local_get(local),
                    FieldTy::Timestamp => sink.local_get(local),
                    FieldTy::Text => {
                        sink.local_get(local);
                        sink.local_get(local + 1)
                    }
                    FieldTy::RecordId { .. } => sink.local_get(local),
                },
                Ty::Record(named) => todo!(),
                Ty::Struct(table_data) => todo!(),
                Ty::Iterator { item_ty, kind } => todo!(),
                Ty::Unit => todo!(),
                Ty::Any => todo!(),
            };

            Some(ty)
        }
        Expr::FnCall {
            name:
                Spanned {
                    value: name,
                    span: name_span,
                },
            args,
        } => match name.as_ref() {
            "hey" => {
                if args.is_empty() {
                    let idx = ctx.global.builder.functions.get("hey")?;
                    ctx.instr(&Instruction::Call(idx));
                    Some(Ty::Field(FieldTy::Text))
                } else {
                    None
                }
            }
            "fetch_record" => {
                if let [arg] = args.as_slice() {
                    let arg_ty = compile_expr(&arg.value, ctx)?;

                    let Ty::Field(FieldTy::RecordId { table_name }) = &arg_ty else {
                        ctx.global.diagnostics.add_error(
                            arg.span,
                            CompileErr::MissmatchedTy {
                                expected: Ty::Field(FieldTy::RecordId {
                                    table_name: "any".into(),
                                }),
                                found: arg_ty,
                            },
                        );
                        return None;
                    };

                    let table_idx = ctx.table_index(table_name);

                    let func_idx = ctx.global.builder.functions.get("fetch_record")?;

                    ctx.instr(&Instruction::I32Const(table_idx.cast_signed()))
                        .instr(&Instruction::Call(func_idx))
                        .instr(&Instruction::I32Const(26));

                    Some(Ty::Field(FieldTy::Text))
                } else {
                    None
                }
            }
            "len" => {
                if let [arg] = args.as_slice() {
                    let arg_ty = compile_expr(&arg.value, ctx)?;

                    let is_slicy_type = is_slicy_type(&arg_ty);

                    if !is_slicy_type {
                        ctx.global.diagnostics.add_error(
                            arg.span,
                            CompileErr::MissmatchedTy {
                                expected: Ty::Field(FieldTy::Text),
                                found: arg_ty,
                            },
                        );
                        return None;
                    };

                    let temp_local = ctx.local.func_builder.i32_temp_local();

                    ctx.instr_sink()
                        .local_set(temp_local)
                        .drop()
                        .local_get(temp_local);

                    Some(Ty::Field(FieldTy::IntI32))
                } else {
                    None
                }
            }
            "ptr" => {
                if let [arg] = args.as_slice() {
                    let arg_ty = compile_expr(&arg.value, ctx)?;

                    let is_slicy_type = is_slicy_type(&arg_ty);

                    if !is_slicy_type {
                        ctx.global.diagnostics.add_error(
                            arg.span,
                            CompileErr::MissmatchedTy {
                                expected: Ty::Field(FieldTy::Text),
                                found: arg_ty,
                            },
                        );
                        return None;
                    };

                    ctx.instr_sink().drop();

                    Some(Ty::Field(FieldTy::IntI32))
                } else {
                    None
                }
            }
            "sum" => {
                if let [arg] = args.as_slice() {
                    let arg_ty = compile_expr(&arg.value, ctx)?;

                    if let Ty::Iterator { item_ty, kind } = &arg_ty
                        && let Ty::Field(FieldTy::IntI32) = item_ty.as_ref()
                    {
                    } else {
                        ctx.global
                            .diagnostics
                            .add_error(arg.span, CompileErr::SumNotCalledOnI32Iter { ty: arg_ty });
                        return None;
                    }

                    let byte_count_per_item = mem_size_of_ty(&Ty::Field(FieldTy::IntI32));

                    let locals = ctx.local.func_builder.local_multi(ValType::I32, 4);
                    let sum_local = locals + 0;
                    let i_local = locals + 1;
                    let ptr_local = locals + 2;
                    let len_local = locals + 3;

                    // let loop_block = ctx.local.func_builder.block();
                    // let body_block = ctx.local.func_builder.block();

                    ctx.instr_sink()
                        .local_set(len_local)
                        .local_set(ptr_local)
                        .i32_const(0)
                        .local_set(sum_local)
                        .i32_const(0)
                        .local_set(i_local);

                    ctx.instr_sink()
                        .block(wasm_encoder::BlockType::Empty)
                        .loop_(wasm_encoder::BlockType::Empty)
                        // i >= n -> break
                        .local_get(i_local)
                        .local_get(len_local)
                        .i32_ge_u()
                        .br_if(1)
                        // body
                        .local_get(i_local)
                        .i32_const(byte_count_per_item.cast_signed())
                        .i32_mul()
                        .local_get(ptr_local)
                        .i32_add()
                        .i32_load(MemArg {
                            offset: 0,
                            align: 0,
                            memory_index: CodeBuilder::MEMORY_INDEX,
                        })
                        .local_get(sum_local)
                        .i32_add()
                        .local_set(sum_local)
                        // i += 1
                        .local_get(i_local)
                        .i32_const(1)
                        .i32_add()
                        .local_set(i_local)
                        .br(0)
                        .end()
                        .end()
                        // return sum
                        .local_get(sum_local);

                    Some(Ty::Field(FieldTy::IntI32))
                } else {
                    None
                }
            }
            _ => None,
        },
        Expr::LambdaFn { args, body } => todo!(),
        Expr::Block(ExprBlock {
            instructions,
            return_expr,
        }) => {
            let mut ctx = ctx.block();

            for instruction in instructions {
                match &instruction.value {
                    crate::expr::Instruction::Let {
                        let_span,
                        name,
                        ty,
                        expr,
                    } => {
                        let value_ty = compile_expr(&expr.value, &mut ctx)?;

                        if let Some(ty) = &ty
                            && ty.value != value_ty
                        {
                            ctx.global.diagnostics.add_error(
                                ty.span,
                                CompileErr::MissmatchedTy {
                                    expected: ty.value.clone(),
                                    found: value_ty,
                                },
                            );
                            return None;
                        }

                        let local_id = match &value_ty {
                            Ty::Field(field_ty) => match field_ty {
                                FieldTy::IntI32 => {
                                    let id = ctx.local.func_builder.local(ValType::I32);

                                    ctx.instr_sink().local_set(id);

                                    id
                                }
                                FieldTy::Bool => {
                                    let id = ctx.local.func_builder.local(ValType::I32);

                                    ctx.instr_sink().local_set(id);

                                    id
                                }
                                FieldTy::Timestamp => {
                                    let id = ctx.local.func_builder.local(ValType::I64);

                                    ctx.instr_sink().local_set(id);

                                    id
                                }
                                FieldTy::Text => {
                                    let id = ctx.local.func_builder.local_multi(ValType::I32, 2);

                                    ctx.instr_sink().local_set(id + 1);
                                    ctx.instr_sink().local_set(id);

                                    id
                                }
                                FieldTy::RecordId { table_name } => {
                                    let id = ctx.local.func_builder.local(ValType::V128);

                                    ctx.instr_sink().local_set(id);

                                    id
                                }
                            },
                            Ty::Record(named) => todo!(),
                            Ty::Struct(table_data) => todo!(),
                            Ty::Iterator { item_ty, kind } => todo!(),
                            Ty::Unit => todo!(),
                            Ty::Any => todo!(),
                        };

                        ctx.set_variable(
                            &name.value,
                            VariableDef {
                                span: name.span,
                                local: local_id,
                                ty: value_ty,
                            },
                        );
                    }
                    crate::expr::Instruction::Return { return_span, expr } => {
                        todo!()
                    }
                    crate::expr::Instruction::Expr { expr } => {
                        println!("ignoring expr instruction in block");
                    }
                }
            }

            if let Some(expr) = return_expr {
                let ty = compile_expr(&expr.value, &mut ctx)?;

                Some(ty)
            } else {
                Some(Ty::Unit)
            }
        }
        Expr::Query(QueryExpr { table_name, filter }) => {
            let table_idx = ctx.table_index(&table_name.value);

            let filter_func_callback_id = match filter {
                Some(filter) => match &filter.value {
                    Expr::LambdaFn { args, body } => {
                        let mut func_builder = FunctionBuilder::new(vec![FieldTy::IntI32]);
                        let mut scopes = Vec::new();

                        let mut inner_ctx = ctx.block();
                        inner_ctx.local = LocalCtx {
                            func_builder: &mut func_builder,
                            scopes: &mut scopes,
                        };
                        inner_ctx.scope = 0;

                        let Some(result_ty) = compile_expr(&body.value, &mut inner_ctx) else {
                            ctx.global.diagnostics.add_error(
                                table_name.span,
                                CompileErr::Custom("error while building lambda expr".to_owned()),
                            );
                            return None;
                        };

                        if result_ty != Ty::Field(FieldTy::Bool) {
                            ctx.global.diagnostics.add_error(
                                table_name.span,
                                CompileErr::Custom("lambda fn does not return bool".to_owned()),
                            );
                            return None;
                        }

                        func_builder.instr_sink().end();
                        func_builder.set_return_type(Ty::Field(FieldTy::Bool));

                        let func = func_builder.finish().unwrap();

                        let func_idx = ctx.global.builder.functions.push(func);

                        let callback_id = ctx.global.builder.functions.register_callback(func_idx);

                        callback_id
                    }
                    _ => {
                        ctx.global.diagnostics.add_error(
                            table_name.span,
                            CompileErr::Custom("filter is not a lambda expr".to_owned()),
                        );

                        return None;
                    }
                },
                None => {
                    ctx.global.diagnostics.add_error(
                        table_name.span,
                        CompileErr::Custom("no filter set".to_owned()),
                    );
                    return None;
                }
            };

            let iter_table_func_idx = ctx.global.builder.functions.get("iter_table").unwrap();

            ctx.local
                .func_builder
                .instr_sink()
                .i32_const(filter_func_callback_id.cast_signed())
                .i32_const(table_idx.cast_signed())
                .call(iter_table_func_idx);

            Some(Ty::Field(FieldTy::IntI32))
        }
        Expr::EmptyPlaceholder => todo!(),
    }
}

enum FieldAccessResult {
    StructWithOffset { ty: Ty, offset: u32 },
    Value(Ty),
}

impl FieldAccessResult {
    fn ty(&self) -> &Ty {
        match self {
            FieldAccessResult::StructWithOffset { ty, .. } => ty,
            FieldAccessResult::Value(ty) => ty,
        }
    }

    fn get(&self, func: &mut FunctionBuilder) {
        match self {
            FieldAccessResult::StructWithOffset { ty, offset } => match ty {
                Ty::Field(field_ty) => match field_ty {
                    FieldTy::IntI32 => {
                        func.instr(&Instruction::I32Load(MemArg {
                            offset: *offset as u64,
                            align: 0,
                            memory_index: CodeBuilder::MEMORY_INDEX,
                        }));
                    }
                    FieldTy::Bool => {
                        func.instr(&Instruction::I32Load8U(MemArg {
                            offset: *offset as u64,
                            align: 0,
                            memory_index: CodeBuilder::MEMORY_INDEX,
                        }));
                    }
                    FieldTy::Timestamp => {
                        func.instr(&Instruction::I64Load(MemArg {
                            offset: *offset as u64,
                            align: 0,
                            memory_index: CodeBuilder::MEMORY_INDEX,
                        }));
                    }
                    FieldTy::Text => {
                        let temp_local = func.i32_temp_local();
                        func.instr_sink()
                            .local_tee(temp_local)
                            .i32_load(MemArg {
                                offset: *offset as u64,
                                align: 0,
                                memory_index: CodeBuilder::MEMORY_INDEX,
                            })
                            .local_get(temp_local)
                            .i32_add()
                            .local_get(temp_local)
                            .i32_load(MemArg {
                                offset: *offset as u64 + 4,
                                align: 0,
                                memory_index: CodeBuilder::MEMORY_INDEX,
                            });
                    }
                    FieldTy::RecordId { table_name: _ } => {
                        func.instr(&Instruction::V128Load(MemArg {
                            offset: *offset as u64,
                            align: 0,
                            memory_index: CodeBuilder::MEMORY_INDEX,
                        }));
                    }
                },
                _ => todo!(),
            },
            FieldAccessResult::Value(_) => (),
        }
    }
}

fn compile_field_access(expr: &Expr, ctx: &mut Ctx<'_>) -> Option<FieldAccessResult> {
    match expr {
        Expr::FieldAccess {
            value,
            dot_span,
            field,
        } => {
            let value = compile_field_access(&value.value, ctx)?;

            match value.ty() {
                Ty::Field(FieldTy::RecordId { table_name }) => {
                    value.get(&mut ctx.local.func_builder);

                    let Some(table_data) = ctx.global.tables.get(table_name) else {
                        ctx.global.diagnostics.add_error(
                            *dot_span,
                            CompileErr::UnkownTable {
                                table_name: table_name.clone(),
                            },
                        );
                        return None;
                    };

                    let Some(Spanned {
                        value: field_name,
                        span: field_name_span,
                    }) = &field
                    else {
                        return None;
                    };

                    ctx.global.diagnostics.add_completion(Spanned::new(
                        *field_name_span,
                        CompletionHint {
                            options: table_data
                                .fields()
                                .map(|field| field.name.as_ref().into())
                                .collect(),
                        },
                    ));

                    let Some(field_data) = table_data.field(&field_name) else {
                        ctx.global.diagnostics.add_error(
                            *dot_span,
                            CompileErr::UnkownTableField {
                                field: field_name.clone(),
                                table_name: table_name.clone(),
                            },
                        );
                        return None;
                    };

                    let table_idx = ctx.table_index(table_name);

                    let func_idx = ctx.global.builder.functions.get("fetch_record")?;

                    ctx.local
                        .func_builder
                        .instr(&Instruction::I32Const(table_idx.cast_signed()))
                        .instr(&Instruction::Call(func_idx));

                    Some(FieldAccessResult::StructWithOffset {
                        ty: Ty::Field(field_data.ty.clone()),
                        offset: field_data.offset,
                    })
                }
                ty => {
                    ctx.global.diagnostics.add_error(
                        *dot_span,
                        CompileErr::FieldAccessOnInvalidTy { ty: ty.clone() },
                    );
                    return None;
                }
            }
        }
        expr => compile_expr(expr, ctx).map(FieldAccessResult::Value),
    }
}

fn mem_size_of_ty(ty: &Ty) -> u32 {
    match &ty {
        Ty::Field(field_ty) => match field_ty {
            FieldTy::IntI32 => 4,
            FieldTy::Bool => 1,
            FieldTy::Timestamp => 8,
            FieldTy::Text => 8,
            FieldTy::RecordId { table_name } => 16,
        },
        Ty::Record(named) => todo!(),
        Ty::Struct(table_data) => todo!(),
        Ty::Iterator { item_ty, kind } => todo!(),
        Ty::Unit => todo!(),
        Ty::Any => todo!(),
    }
}

fn is_slicy_type(ty: &Ty) -> bool {
    match &ty {
        Ty::Field(FieldTy::Text) => true,
        Ty::Iterator {
            item_ty,
            kind: IterKind::Array,
        } if item_ty.as_ref() == &Ty::Field(FieldTy::IntI32) => true,
        _ => false,
    }
}
