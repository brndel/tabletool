use std::time::Instant;

use wasm_encoder::{
    CodeSection, EntityType, ExportSection, FunctionSection, ImportSection, TypeSection,
    ValType,
};
use wasmer::{
    Function, Instance, Store, imports, sys::NativeEngineExt,
};
use wasmer_types::CompilationProgressCallback;

fn main() {
    let module = build_module();

    run_wasmer(&module);

}

fn build_module() -> Vec<u8> {
    let mut module = wasm_encoder::Module::new();

    let mut types_section = TypeSection::new();
    let host_func_ty_idx = types_section.len();
    types_section.ty().function([ValType::I32], []);
    let add_one_ty_idx = types_section.len();
    types_section.ty().function([ValType::I32], [ValType::I32]);

    module.section(&types_section);

    let mut import_section = ImportSection::new();
    let host_func_idx = import_section.len();
    import_section.import("host", "host_func", EntityType::Function(host_func_ty_idx));

    module.section(&import_section);


    let mut function_section = FunctionSection::new();
    let add_one_fn_idx = function_section.len() + 1;
    function_section.function(add_one_ty_idx);
    module.section(&function_section);

    let mut export_section = ExportSection::new();
    export_section.export("add_one", wasm_encoder::ExportKind::Func, add_one_fn_idx);
    module.section(&export_section);

    let mut code_section = CodeSection::new();
    let mut add_one_fn = wasm_encoder::Function::new([(1, ValType::I32)]);

    add_one_fn
        .instructions()
        .local_get(0)
        .i32_const(1)
        .i32_add()
        .local_tee(1)
        .call(host_func_idx)
        .local_get(1)
        .end();

    code_section.function(&add_one_fn);
    // code_section.function(type_index);

    module.section(&code_section);

    module.finish()
}

fn run_wasmer(module: &[u8]) {
    let _timer = ScopeTimer::start("wasmer");

    let engine = wasmer::Engine::default();

    let mut store = Store::new(engine);

    let module = {
        let _timer = ScopeTimer::start("wasmer parse");

        let module_wat = r#"
        (module
          (import "host" "host_func" (func $host_hello (param i32)))
        
          (type $t0 (func (param i32) (result i32)))
          (func $add_one (export "add_one") (type $t0) (param $p0 i32) (result i32)
            local.get $p0
            i32.const 1
            i32.add
            local.tee 0
            call $host_hello
            local.get 0
          )
        )
        "#;

        let callback = CompilationProgressCallback::new(|p| {
            let percent = p.phase_step().unwrap_or(0) as f32 / p.phase_step_count().unwrap_or(0) as f32;
            println!("compile progress: {:.1}%", percent * 100.0);
            Ok(())
        });

        let module = store.engine().new_module_with_progress(&module, callback);
        // Module::from_binary(&store, &module).unwrap()
        module.unwrap()
    };

    let _timer = ScopeTimer::start("wasmer link");

    fn host_hello(num: i32) {
        println!("Hello from host: {}", num);
    }

    let import_object = imports! {
        "host" => {
            "host_func" => Function::new_typed(&mut store, host_hello)
        }
    };

    let instance = Instance::new(&mut store, &module, &import_object).unwrap();

    let add_one = instance
        .exports
        .get_typed_function::<i32, i32>(&store, "add_one")
        .unwrap();

    let _timer = ScopeTimer::start("wasmer run");
    let result = add_one.call(&mut store, 42).unwrap();

    println!("got {result} as result")
}

struct ScopeTimer {
    name: &'static str,
    start: Instant,
}

impl ScopeTimer {
    pub fn start(name: &'static str) -> Self {
        println!("TIMER '{name}' START");
        Self {
            name,
            start: Instant::now(),
        }
    }

    pub fn end(self) {
        drop(self);
    }
}

impl Drop for ScopeTimer {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed();

        println!(
            "TIMER '{}' took {}.{:03}ms",
            self.name,
            elapsed.as_millis(),
            elapsed.as_micros() % 1000
        );
    }
}
