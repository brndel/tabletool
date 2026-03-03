use std::{borrow::Cow, collections::HashMap, iter, ops::Range, sync::Arc};

use ulid::Ulid;

use crate::{
    asm_code::{
        asm_code::{AccessTableIdx, IntBits},
        pointer::{AsmPointer, AsmSlicePointer, Namespace},
        program::Program,
    },
    ty::{FieldTy, Ty},
    value::{FieldValue, Value},
};

pub struct AsmRuntime<'code, 'record, Q: QueryProvider> {
    instruction_pointer: usize,
    program: &'code Program,
    stack_pointer: u32,
    stack: Vec<u8>,
    heap: Vec<u8>,
    query: &'record mut Q,
    records: Vec<Cow<'record, [u8]>>,
    record_index: HashMap<AccessTableIdx, RecordIndex>,
    panic_message: Option<String>,
}

#[derive(Default)]
struct RecordIndex {
    /// (record_index in AsmRuntime::records, iterator len -- 0 if its no iterator)
    record_idx: Option<(u16, u32)>,
    id_records: HashMap<Ulid, u16>,
}

pub trait QueryProvider {
    fn get_record(&mut self, table_idx: AccessTableIdx, id: Ulid) -> Option<Vec<u8>>;
}

impl QueryProvider for () {
    fn get_record(&mut self, _table_name: AccessTableIdx, _id: Ulid) -> Option<Vec<u8>> {
        None
    }
}

impl<'code, 'record, Q: QueryProvider> AsmRuntime<'code, 'record, Q> {
    pub fn new(
        program: &'code Program,
        records: Vec<Cow<'record, [u8]>>,
        query: &'record mut Q,
        record_index: impl IntoIterator<Item = (AccessTableIdx, u16, u32)>,
    ) -> Self {
        Self {
            instruction_pointer: 0,
            program,
            stack_pointer: 0,
            stack: Vec::new(),
            heap: Vec::new(),
            query,
            records,
            record_index: HashMap::from_iter(record_index.into_iter().map(
                |(table_idx, record_idx, record_iter_len)| {
                    (
                        table_idx,
                        RecordIndex {
                            record_idx: Some((record_idx, record_iter_len)),
                            id_records: Default::default(),
                        },
                    )
                },
            )),
            panic_message: None,
        }
    }

    pub fn run(&mut self) {
        for (pointer, instruction) in self.program.code.iter().enumerate() {
            println!("[{:04}] {:?}", pointer, instruction);
        }
        println!("----- START -----");
        while self.instruction_pointer < self.program.code.len() && self.panic_message.is_none() {
            let instruction = &self.program.code[self.instruction_pointer];
            println!("[{:04}] {:?}", self.instruction_pointer, instruction);

            self.instruction_pointer += 1;

            instruction.exec(self);
        }
    }

    pub fn result(self) -> Result<Value, String> {
        if let Some(panic) = self.panic_message {
            return Err(panic);
        }

        match &self.program.return_ty {
            Ty::Field(field_ty) => match field_ty {
                FieldTy::IntI32 => {
                    let result = &self.stack[0..(IntBits::I32.bytes() as usize)];
                    let result = i32::from_be_bytes(result.try_into().unwrap());
                    Ok(Value::Field(FieldValue::Int(result)))
                }
                FieldTy::Bool => {
                    let result = self.stack[0];
                    let result = if result == 0 { false } else { true };
                    Ok(Value::Field(FieldValue::Bool(result)))
                }
                FieldTy::Timestamp => todo!(),
                FieldTy::Text => {
                    let result = &self.stack[0..(AsmSlicePointer::BYTES as usize)];
                    let result_pointer = AsmSlicePointer::from_bytes(result.try_into().unwrap());
                    let result_bytes = self.get(&result_pointer.pointer, result_pointer.len);

                    let result_str = String::from_utf8_lossy(result_bytes);

                    Ok(Value::Field(FieldValue::Text(result_str.into_owned())))
                }
                FieldTy::RecordId { table_name } => {
                    let result = &self.stack[0..(IntBits::U128.bytes() as usize)];
                    let result = u128::from_be_bytes(result.try_into().unwrap());
                    Ok(Value::Field(FieldValue::RecordId {
                        id: Ulid(result),
                        table_name: table_name.clone(),
                    }))
                }
            },
            Ty::Record(table) => Err(format!("record type")),
            Ty::Iterator { item_ty, kind } => Err(format!("iter type")),
            Ty::Any => Err(format!("any type")),
        }
    }

    pub fn result_bool(&self) -> bool {
        let result = self.stack[0];
        if result == 0 { false } else { true }
    }

    pub fn result_i32(&self) -> i32 {
        let result = &self.stack[0..((i32::BITS / 8) as usize)];
        i32::from_be_bytes(result.try_into().unwrap())
    }

    fn get_mem_range(&self, mut offset: u32, len: u32, is_stack: bool) -> Range<usize> {
        if is_stack {
            offset += self.stack_pointer;
        }

        (offset as usize)..((offset + len) as usize)
    }

    pub fn get(&self, pointer: &AsmPointer, len: u32) -> &[u8] {
        match pointer.namespace {
            Namespace::Stack => &self.stack[self.get_mem_range(pointer.offset, len, true)],
            Namespace::Heap => &self.heap[self.get_mem_range(pointer.offset, len, false)],
            Namespace::Const => {
                &self.program.const_memory[self.get_mem_range(pointer.offset, len, false)]
            }
            Namespace::Record { idx } => {
                &self.records[idx as usize][self.get_mem_range(pointer.offset, len, false)]
            }
        }
    }

    pub fn get_indirect(&self, indirect_ptr: &AsmPointer, len: u32) -> &[u8] {
        let ptr = self.get(indirect_ptr, AsmPointer::BYTES);
        let ptr = AsmPointer::from_bytes(ptr.try_into().unwrap());

        let value = self.get(&ptr, len);

        value
    }

    pub fn set(&mut self, pointer: &AsmPointer, value: &[u8]) {
        let slice = match pointer.namespace {
            Namespace::Stack => {
                let range = self.get_mem_range(pointer.offset, value.len() as u32, true);
                &mut self.stack[range]
            }
            Namespace::Heap => {
                let range = self.get_mem_range(pointer.offset, value.len() as u32, true);
                &mut self.stack[range]
            }
            Namespace::Const => panic!("cannot write to const memory"),
            Namespace::Record { idx: _ } => panic!("cannot write to record memory"),
        };

        slice.copy_from_slice(value);
    }

    pub fn jump(&mut self, target: usize) {
        self.instruction_pointer = target
    }

    pub fn alloc_heap(&mut self, byte_count: usize) -> AsmPointer {
        let offset = self.heap.len() as u32;

        self.heap.extend(iter::repeat(0).take(byte_count as usize));

        AsmPointer {
            namespace: Namespace::Heap,
            offset,
        }
    }

    pub fn reserve_stack(&mut self, byte_count: u32) {
        self.stack.extend(iter::repeat(0).take(byte_count as usize));
    }

    pub fn query_record(&mut self, table_idx: AccessTableIdx, id: Ulid) -> Option<u16> {
        if let Some(index) = self.record_index.get(&table_idx)
            && let Some(index) = index.id_records.get(&id)
        {
            Some(*index)
        } else {
            let bytes = self.query.get_record(table_idx, id)?;
            let idx = self.records.len() as u16;

            self.records.push(Cow::Owned(bytes));

            let index = self.record_index.entry(table_idx).or_default();
            index.id_records.insert(id, idx);

            Some(idx)
        }
    }

    pub fn get_record_idx(&self, table_idx: AccessTableIdx) -> Option<u16> {
        Some(self.record_index.get(&table_idx)?.record_idx?.0)
    }

    pub fn get_record_iter_info(&self, table_idx: AccessTableIdx) -> Option<(u16, u32)> {
        self.record_index.get(&table_idx)?.record_idx
    }

    pub fn panic(&mut self, message: String) {
        if self.panic_message.is_none() {
            println!("PANIC: '{message}'");
            self.panic_message = Some(message);
        }
    }
}
