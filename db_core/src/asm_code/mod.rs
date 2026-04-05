mod asm_code;
mod compile;
mod asm_pointer;
mod program;
mod runtime;
mod asm_iter;
mod err;
mod complier_diagnostics;

pub use asm_code::AccessTableIdx;
pub use compile::compile_expr;
pub use program::Program;
pub use runtime::AsmRuntime;
pub use runtime::QueryProvider;
pub use err::AsmCompileErr;
pub use complier_diagnostics::*;