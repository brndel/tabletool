use std::sync::Arc;

use wasmer::AsEngineRef;

use crate::ty::Ty;

pub struct Program {
    module: Vec<u8>,
    table_indices: Vec<Arc<str>>,
    return_ty: Ty,
}

impl Program {
    pub(super) fn new(
        module: wasm_encoder::Module,
        return_ty: Ty,
        table_indices: Vec<Arc<str>>,
    ) -> Self {
        Self {
            module: module.finish(),
            table_indices,
            return_ty,
        }
    }

    pub fn compile(
        self,
        engine: &impl AsEngineRef,
    ) -> Result<CompiledProgram, wasmer::CompileError> {
        let module = wasmer::Module::new(engine, &self.module)?;

        Ok(CompiledProgram {
            module,
            return_ty: self.return_ty,
            table_indices: self.table_indices,
        })
    }
}

pub struct CompiledProgram {
    pub module: wasmer::Module,
    pub return_ty: Ty,
    pub table_indices: Vec<Arc<str>>,
}
