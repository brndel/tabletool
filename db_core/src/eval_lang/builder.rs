use std::{collections::{HashMap, HashSet}, sync::Arc};

use wasm_encoder::{
    ConstExpr, DataSection, Encode, EntityType, FuncType, Function, GlobalType, Instruction,
    InstructionSink, MemoryType, Module, RefType, SectionId, TableType, TagType, ValType,
};

use crate::{
    eval_lang::program::Program,
    ty::{FieldTy, Ty},
};

pub struct CodeBuilder {
    pub const_memory: ConstMemoryBuilder,
    pub functions: FunctionRegistry,
    pub table_indices: Vec<Arc<str>>,
}

impl CodeBuilder {
    pub const MEMORY_INDEX: u32 = 0;

    pub const HEAP_BASE_GLOBAL: u32 = 0;
    pub const HEAP_GLOBAL: u32 = 1;

    // pub const IMPORT_FUNCTIONS: &'static [&str] = &["fetch_record", "trace"];
    // pub const FETCH_RECORD_IDX: u32 = 0;
    // pub const TRACE_FN_IDX: u32 = 1;
    // pub const IMPORT_FUNCTION_COUNT: u32 = Self::IMPORT_FUNCTIONS.len() as u32;

    // pub const ALLOC_FUNCTION_IDX: u32 = Self::IMPORT_FUNCTION_COUNT + 0;
    // pub const HEY_FUNCTION_IDX: u32 = Self::IMPORT_FUNCTION_COUNT + 1;

    pub const CALLBACK_TABLE: u32 = 0;
    pub const CALLBACK_TABLE_EXPORT_NAME: &'static str = "callback_table";

    pub const MAIN_FN_NAME: &'static str = "main";

    pub const NONE_EXCEPTION_IDX: u32 = 0;

    pub fn finish(self) -> Program {
        let mut type_section = wasm_encoder::TypeSection::new();
        let mut import_section = wasm_encoder::ImportSection::new();
        let mut export_section = wasm_encoder::ExportSection::new();
        let mut func_section = wasm_encoder::FunctionSection::new();
        let mut memory_section = wasm_encoder::MemorySection::new();
        let mut global_section = wasm_encoder::GlobalSection::new();
        let mut code_section = wasm_encoder::CodeSection::new();
        let mut table_section = wasm_encoder::TableSection::new();
        let mut element_section = wasm_encoder::ElementSection::new();
        let data_section_end_offset = self.const_memory.offset;
        let data_section = self.const_memory.finish();

        memory_section.memory(MemoryType {
            minimum: 1,
            maximum: None,
            memory64: false,
            shared: false,
            page_size_log2: None,
        });
        export_section.export(
            "memory",
            wasm_encoder::ExportKind::Memory,
            Self::MEMORY_INDEX,
        );
        export_section.export(
            Self::CALLBACK_TABLE_EXPORT_NAME,
            wasm_encoder::ExportKind::Table,
            Self::CALLBACK_TABLE,
        );

        global_section.global(
            GlobalType {
                val_type: ValType::I32,
                mutable: false,
                shared: false,
            },
            &ConstExpr::i32_const(data_section_end_offset),
        );
        global_section.global(
            GlobalType {
                val_type: ValType::I32,
                mutable: true,
                shared: false,
            },
            &ConstExpr::i32_const(data_section_end_offset),
        );

        table_section.table(TableType {
            element_type: RefType::FUNCREF,
            table64: false,
            minimum: 4,
            maximum: None,
            shared: false,
        });

        element_section.active(Some(Self::CALLBACK_TABLE), &ConstExpr::i32_const(0), wasm_encoder::Elements::Functions(std::borrow::Cow::Borrowed(&self.functions.callbacks)));

        let mut func_ty_counter = 0;

        let mut main_fn_return_ty = None;

        for (idx, func) in self.functions.functions.iter().enumerate() {
            match func {
                RegistryFunc::Imported {
                    name,
                    ty,
                    return_type,
                } => {
                    type_section.ty().func_type(&ty);
                    import_section.import("db", name, EntityType::Function(func_ty_counter));
                    func_ty_counter += 1;
                }
                RegistryFunc::Local {
                    func:
                        BuiltFunction {
                            func,
                            ty,
                            return_type,
                            export_name,
                        },
                } => {
                    type_section.ty().func_type(&ty);
                    func_section.function(func_ty_counter);
                    code_section.function(&func);

                    func_ty_counter += 1;

                    if let Some(export_name) = export_name {
                        main_fn_return_ty = Some(return_type);
                        export_section.export(
                            &export_name,
                            wasm_encoder::ExportKind::Func,
                            idx as u32,
                        );
                    }
                }
            }
        }

        // { // None exception
        //     tag_section.tag(TagType {
        //         kind: wasm_encoder::TagKind::Exception,
        //         func_type_idx: func_ty_counter,
        //     });

        //     type_section.ty().func_type(&FuncType::new(
        //         [],
        //         [],
        //     ));

        //     func_ty_counter += 1;
        // }

        let mut module = Module::new();

        module
            .section(&type_section)
            .section(&import_section)
            .section(&func_section)
            .section(&table_section)
            .section(&memory_section)
            .section(&global_section)
            .section(&export_section)
            .section(&element_section)
            .section(&code_section)
            .section(&data_section);

        Program::new(
            module,
            main_fn_return_ty.expect("missing main fn").clone(),
            self.table_indices,
        )
    }
}

#[derive(Default)]
pub struct ConstMemoryBuilder {
    offset: i32,
    data: DataSection,
}

impl ConstMemoryBuilder {
    pub fn new() -> Self {
        Self {
            offset: 16,
            data: DataSection::new(),
        }
    }

    pub fn alloc_const(&mut self, data: &[u8]) -> (i32, i32) {
        let offset = self.offset;
        self.data.active(
            CodeBuilder::MEMORY_INDEX,
            &ConstExpr::i32_const(offset),
            data.iter().copied(),
        );

        let len = data.len() as i32;
        self.offset += len;

        return (offset, len);
    }

    pub fn finish(self) -> DataSection {
        self.data
    }
}

pub struct FunctionBuilder {
    export_name: Option<Arc<str>>,
    locals: Vec<(u32, ValType)>,
    local_counter: u32,
    i32_temp_local: Option<u32>,
    i64_temp_local: Option<u32>,
    v128_temp_local: Option<u32>,

    block_counter: u32,

    params: Vec<ValType>,
    return_ty: Option<Ty>,

    func_bytes: Vec<u8>,
}

impl FunctionBuilder {
    pub fn new(params: Vec<FieldTy>) -> Self {
        let params: Vec<_> = Self::ty_to_val_ty(params.into_iter()).collect();

        Self {
            export_name: None,
            locals: Vec::new(),
            local_counter: params.len() as u32,
            i32_temp_local: None,
            i64_temp_local: None,
            v128_temp_local: None,

            block_counter: 0,

            params,
            return_ty: None,

            func_bytes: Vec::new(),
        }
    }

    fn ty_to_val_ty(types: impl Iterator<Item = FieldTy>) -> impl Iterator<Item = ValType> {
        types
            .flat_map(|ty| match ty {
                FieldTy::IntI32 => &[ValType::I32] as &[ValType],
                FieldTy::Bool => &[ValType::I32],
                FieldTy::Timestamp => &[ValType::I64],
                FieldTy::Text => &[ValType::I32, ValType::I32],
                FieldTy::RecordId { .. } => &[ValType::V128],
            })
            .cloned()
    }

    pub fn set_export_name(&mut self, name: Arc<str>) {
        self.export_name = Some(name);
    }

    pub fn set_return_type(&mut self, ty: Ty) {
        self.return_ty = Some(ty);
    }

    pub fn instr(&mut self, instr: &Instruction) -> &mut Self {
        // println!("INSTR {instr:?}");
        instr.encode(&mut self.func_bytes);

        self
    }

    pub fn instr_sink(&mut self) -> InstructionSink<'_> {
        InstructionSink::new(&mut self.func_bytes)
    }

    pub fn block(&mut self) -> u32 {
        let block = self.block_counter;
        self.block_counter += 1;

        block
    }

    pub fn i32_temp_local(&mut self) -> u32 {
        if let Some(local) = self.i32_temp_local {
            local
        } else {
            self.local(ValType::I32)
        }
    }

    pub fn i64_temp_local(&mut self) -> u32 {
        if let Some(local) = self.i64_temp_local {
            local
        } else {
            self.local(ValType::I64)
        }
    }

    pub fn v128_temp_local(&mut self) -> u32 {
        if let Some(local) = self.v128_temp_local {
            local
        } else {
            self.local(ValType::V128)
        }
    }

    pub fn local(&mut self, ty: ValType) -> u32 {
        self.local_multi(ty, 1)
    }

    pub fn local_multi(&mut self, ty: ValType, amount: u32) -> u32 {
        if let Some((count, last_local)) = self.locals.last_mut()
            && *last_local == ty
        {
            *count += amount;
        } else {
            self.locals.push((amount, ty));
        }

        let local = self.local_counter;
        self.local_counter += amount;

        local
    }

    pub fn finish(self) -> Option<BuiltFunction> {
        let return_ty = self.return_ty?;

        let result_ty: &[ValType] = match &return_ty {
            Ty::Field(field_ty) => match field_ty {
                FieldTy::IntI32 => &[ValType::I32],
                FieldTy::Bool => &[ValType::I32],
                FieldTy::Timestamp => &[ValType::I64],
                FieldTy::Text => &[ValType::I32, ValType::I32],
                FieldTy::RecordId { .. } => &[ValType::V128],
            },
            _ => todo!(),
        };

        let func_type = FuncType::new(self.params, result_ty.iter().cloned());

        let mut func = Function::new(self.locals);
        func.raw(self.func_bytes);

        Some(BuiltFunction {
            func,
            ty: func_type,
            return_type: return_ty,
            export_name: self.export_name,
        })
    }
}

pub struct BuiltFunction {
    func: Function,
    ty: FuncType,
    return_type: Ty,
    export_name: Option<Arc<str>>,
}

pub enum FuncIdent {
    Name { name: Arc<str> },
    Position { offset: usize },
}

pub struct FunctionRegistry {
    functions: Vec<RegistryFunc>,
    callbacks: Vec<u32>
}

enum RegistryFunc {
    Imported {
        name: &'static str,
        ty: FuncType,
        return_type: Ty,
    },
    Local {
        func: BuiltFunction,
    },
}

mod builtin_funcs {
    use wasm_encoder::{MemArg, ValType};

    use crate::{
        eval_lang::builder::{BuiltFunction, CodeBuilder, FunctionBuilder},
        ty::{FieldTy, Ty},
    };

    pub fn alloc() -> BuiltFunction {
        let mut builder = FunctionBuilder::new(vec![FieldTy::IntI32]);
        builder.set_return_type(Ty::Field(FieldTy::IntI32));
        builder.set_export_name("alloc".into());

        let heap_temp = builder.local(ValType::I32);

        builder
            .instr_sink()
            .global_get(CodeBuilder::HEAP_GLOBAL)
            .local_tee(heap_temp)
            .local_get(0)
            .i32_add()
            .global_set(CodeBuilder::HEAP_GLOBAL)
            .local_get(heap_temp)
            .end();

        builder.finish().unwrap()
    }

    pub fn hey_ptr(alloc_func: u32) -> BuiltFunction {
        let mut builder = FunctionBuilder::new(vec![]);
        builder.set_return_type(Ty::Field(FieldTy::Text));
        builder.set_export_name("hey".into());

        let start_ptr = builder.local(ValType::I32);

        let needed_bytes = 3;
        let aligned_bytes = 4;

        builder
            .instr_sink()
            .i32_const(aligned_bytes)
            .call(alloc_func)
            .local_set(start_ptr)
            // H
            .local_get(start_ptr)
            .i32_const(b'H' as i32)
            .i32_store8(MemArg {
                offset: 0,
                align: 0,
                memory_index: CodeBuilder::MEMORY_INDEX,
            })
            // e
            .local_get(start_ptr)
            .i32_const(b'H' as i32)
            .i32_store8(MemArg {
                offset: 1,
                align: 0,
                memory_index: CodeBuilder::MEMORY_INDEX,
            })
            // y
            .local_get(start_ptr)
            .i32_const(b'H' as i32)
            .i32_store8(MemArg {
                offset: 2,
                align: 0,
                memory_index: CodeBuilder::MEMORY_INDEX,
            })
            // return
            .local_get(start_ptr)
            .i32_const(needed_bytes)
            .end();

        builder.finish().unwrap()
    }
}

impl FunctionRegistry {
    pub fn new() -> Self {
        let mut this = Self {
            functions: vec![
                RegistryFunc::Imported {
                    name: "fetch_record",
                    ty: FuncType::new([ValType::V128, ValType::I32], [ValType::I32]),
                    return_type: Ty::Any,
                },
                RegistryFunc::Imported {
                    name: "trace",
                    ty: FuncType::new([], []),
                    return_type: Ty::Unit,
                },
                RegistryFunc::Imported {
                    name: "iter_table",
                    ty: FuncType::new([ValType::I32, ValType::I32], [ValType::I32]),
                    return_type: Ty::Field(FieldTy::IntI32),
                },
            ],
            callbacks: Vec::new()
        };

        let alloc_func = this.push(builtin_funcs::alloc());
        this.push(builtin_funcs::hey_ptr(alloc_func));

        this
    }

    pub fn push(&mut self, func: BuiltFunction) -> u32 {
        let idx = self.functions.len() as u32;

        self.functions.push(RegistryFunc::Local { func });

        idx
    }

    pub fn get(&self, name: &str) -> Option<u32> {
        self.functions
            .iter()
            .enumerate()
            .find(|(_, func)| match func {
                RegistryFunc::Imported {
                    name: func_name, ..
                } => *func_name == name,
                RegistryFunc::Local {
                    func:
                        BuiltFunction {
                            export_name: Some(func_name),
                            ..
                        },
                } => func_name.as_ref() == name,
                RegistryFunc::Local { .. } => false,
            })
            .map(|(idx, _)| idx as u32)
    }

    pub fn register_callback(&mut self, func_idx: u32) -> u32 {
        let new_callback_id = self.callbacks.len();
        self.callbacks.push(func_idx);

        new_callback_id as u32
    }
}
