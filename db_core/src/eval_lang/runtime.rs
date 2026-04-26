use std::{
    borrow::Cow,
    cell::OnceCell,
    collections::HashMap,
    sync::{Arc, Mutex},
};

use chrono::DateTime;
use ulid::Ulid;
use wasmer::{
    AsStoreMut, AsStoreRef, FunctionEnv, FunctionEnvMut, FunctionType, Memory, Store, Table, Type, TypedFunction, WasmPtr, imports
};

use crate::{
    eval_lang::{builder::CodeBuilder, program::CompiledProgram},
    ty::{FieldTy, Ty},
    value::{self, Value},
};

pub struct EvalRuntime<R> {
    record_provider: R,
}

impl<R> EvalRuntime<R> {
    pub fn new(record_provider: R) -> Self {
        Self { record_provider }
    }
}

pub trait RecordProvider {
    fn fetch_record(&self, table_name: &str, record: Ulid) -> Option<Cow<'_, [u8]>>;

    fn iter_table(&self, table_name: &str, f: impl FnMut(Ulid, Cow<'_, [u8]>));
}

impl RecordProvider for () {
    fn fetch_record(&self, table_name: &str, record: Ulid) -> Option<Cow<'_, [u8]>> {
        println!("Fetching record {} for {}", record, table_name);
        None
    }

    fn iter_table(&self, table_name: &str, f: impl FnMut(Ulid, Cow<'_, [u8]>)) {
        println!("Iterating over table {}", table_name);
        drop(f);
    }
}

struct Env<R: RecordProvider> {
    memory: Option<Memory>,
    alloc: Option<wasmer::TypedFunction<u32, u32>>,
    callback_table: Option<wasmer::Table>,
    record_cache: HashMap<(Ulid, u32), i32>,
    record_provider: R,
}

impl<R> EvalRuntime<R>
where
    R: RecordProvider + Send + Sync + 'static,
{
    pub fn run_program(self, mut store: impl AsStoreMut, program: &CompiledProgram) -> Value {
        let env = FunctionEnv::new(
            &mut store,
            Env {
                memory: None,
                alloc: None,
                callback_table: None,
                record_cache: HashMap::new(),
                record_provider: self.record_provider,
            },
        );

        let table_indices = Arc::new(program.table_indices.clone());
        // let record_provider = self.record_provider.clone();
        // let mut record_cache = Mutex::new(HashMap::new());

        let fetch_record = wasmer::Function::new_with_env(
            &mut store,
            &env,
            FunctionType::new([Type::V128, Type::I32], [Type::I32]),
            {
                let table_indices = table_indices.clone();
                move |mut env: FunctionEnvMut<Env<R>>, id| {
                    let (env, mut store) = env.data_and_store_mut();

                    let [wasmer::Value::V128(id), wasmer::Value::I32(table_idx)] = id else {
                        unreachable!()
                    };
                    // let id: u128 = unsafe { mem::transmute((id_lower, id_higher)) };
                    let id = Ulid(*id);
                    let table_idx = table_idx.cast_unsigned();

                    {
                        let record_cache = &mut env.record_cache;
                        if let Some(result_addr) = record_cache.get(&(id, table_idx)) {
                            return Ok(vec![wasmer::Value::I32(*result_addr)]);
                        }
                    }

                    let table = table_indices
                        .get(table_idx as usize)
                        .expect("unkown table idx");

                    let record = env.record_provider.fetch_record(table, id);

                    let result_addr = match record {
                        Some(record) => {
                            let alloc = env.alloc.as_ref().unwrap();

                            let memory = env.memory.as_ref().unwrap();

                            let str_len = record.len() as u32;

                            let result_addr = alloc.call(&mut store, str_len).unwrap();

                            let mem_view = memory.view(&store);

                            let ptr = WasmPtr::<u8>::new(result_addr);
                            let mem_slice = ptr.slice(&mem_view, str_len).unwrap();

                            mem_slice.write_slice(&record).unwrap();

                            result_addr.cast_signed()
                        }
                        None => 0,
                    };

                    env.record_cache.insert((id, table_idx), result_addr);

                    Ok(vec![wasmer::Value::I32(result_addr)])
                }
            },
        );

        let iter_table = wasmer::Function::new_with_env(
            &mut store,
            &env,
            FunctionType::new([Type::I32, Type::I32], [Type::I32]),
            move |mut env: FunctionEnvMut<Env<R>>, id| {
                let (env, mut store) = env.data_and_store_mut();

                let [
                    wasmer::Value::I32(callback_id),
                    wasmer::Value::I32(table_idx),
                ] = id
                else {
                    unreachable!()
                };

                let wasm_callback: TypedFunction<u32, u32> = {
                    let table = env.callback_table.as_ref().unwrap();

                    let entry = table.get(&mut store, callback_id.cast_unsigned());

                    match entry {
                        Some(wasmer::Value::FuncRef(Some(f))) => f.typed(&store).unwrap(),
                        _ => panic!(),
                    }
                };

                let table_idx = table_idx.cast_unsigned();
                let table = table_indices
                    .get(table_idx as usize)
                    .expect("unkown table idx");

                // let mut
                let mem = env.memory.as_ref().unwrap();
                let alloc = env.alloc.as_ref().unwrap();

                let mut record_ptr = OnceCell::new();

                let mut counter = 0;

                env.record_provider.iter_table(table, |id, data| {
                    println!("iter {id}");

                    if let Some((_, capacity)) = record_ptr.get() {
                        if *capacity < data.len() {
                            record_ptr.take();
                        }
                    }

                    let (offset, _capacity) = record_ptr.get_or_init(|| {
                        let capacity = data.len();
                        let ptr = alloc.call(&mut store, capacity as u32).unwrap();

                        (ptr, capacity)
                    });

                    let mem_view = mem.view(&mut store);

                    mem_view.write(*offset as u64, data.as_ref()).unwrap();
                    
                    let result = wasm_callback.call(&mut store, *offset).unwrap();

                    if result == 1 {
                        counter += 1;
                    }
                });

                Ok(vec![wasmer::Value::I32(counter)])
            },
        );

        let trace = wasmer::Function::new_typed(&mut store, || println!("TRACE"));

        let imports = imports! {
            "db" => {
                "fetch_record" => fetch_record,
                "iter_table" => iter_table,
                "trace" => trace,
            }
        };

        let instance = wasmer::Instance::new(&mut store, &program.module, &imports).unwrap();

        {
            let memory = instance.exports.get_memory("memory").unwrap().clone();
            let alloc = instance
                .exports
                .get_typed_function(&store, "alloc")
                .unwrap();
            let callback_table = instance.exports.get_table(CodeBuilder::CALLBACK_TABLE_EXPORT_NAME).unwrap().clone();

            let env = env.as_mut(&mut store);
            env.memory = Some(memory);
            env.alloc = Some(alloc);
            env.callback_table = Some(callback_table);
        }

        let main_func = instance
            .exports
            .get_function(CodeBuilder::MAIN_FN_NAME)
            .unwrap();

        let result = main_func.call(&mut store, &[]).unwrap();

        let mem = env.as_ref(&store).memory.as_ref().unwrap().clone();

        Self::get_value(&result, &program.return_ty, store, &mem)
    }
}

impl<R> EvalRuntime<R> {
    fn get_value(
        value: &[wasmer::Value],
        ty: &Ty,
        store: impl AsStoreRef,
        memory: &Memory,
    ) -> value::Value {
        match ty {
            Ty::Field(field_ty) => match field_ty {
                FieldTy::IntI32 => {
                    if let [wasmer::Value::I32(value)] = *value {
                        value::Value::Field(value::FieldValue::Int(value))
                    } else {
                        panic!()
                    }
                }
                FieldTy::Bool => {
                    if let [wasmer::Value::I32(value)] = *value {
                        value::Value::Field(value::FieldValue::Bool(value == 1))
                    } else {
                        panic!()
                    }
                }
                FieldTy::Timestamp => {
                    if let [wasmer::Value::I64(value)] = *value {
                        value::Value::Field(value::FieldValue::Timestamp(
                            DateTime::from_timestamp(value, 0).unwrap(),
                        ))
                    } else {
                        panic!()
                    }
                }
                FieldTy::Text => {
                    if let [wasmer::Value::I32(ptr), wasmer::Value::I32(len)] = *value {
                        println!("string result {ptr}:{len}");
                        let view = memory.view(&store);

                        let ptr = WasmPtr::<u8>::new(ptr.cast_unsigned());
                        let result = ptr.read_utf8_string(&view, len.cast_unsigned()).unwrap();

                        value::Value::Field(value::FieldValue::Text(result))
                    } else {
                        panic!()
                    }
                }
                FieldTy::RecordId { table_name } => {
                    if let [wasmer::Value::V128(value)] = *value {
                        value::Value::Field(value::FieldValue::RecordId {
                            id: Ulid(value),
                            table_name: table_name.clone(),
                        })
                    } else {
                        panic!()
                    }
                }
            },
            _ => todo!(),
        }
    }
}
