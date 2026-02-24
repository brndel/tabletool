use std::{iter, ops::Range};

use ulid::Ulid;

use crate::{
    asm_code::{
        asm_code::IntBits,
        pointer::{AsmPointer, Namespace},
        program::Program,
    },
    ty::{FieldTy, Ty},
    value::{FieldValue, Value},
};

pub struct AsmRuntime<'code, 'record> {
    instruction_pointer: usize,
    program: &'code Program,
    stack_pointer: u32,
    stack: Vec<u8>,
    heap: Vec<u8>,
    records: Vec<&'record [u8]>,
}

impl<'code, 'record> AsmRuntime<'code, 'record> {
    pub fn new(program: &'code Program, records: Vec<&'record [u8]>) -> Self {
        Self {
            instruction_pointer: 0,
            program,
            stack_pointer: 0,
            stack: Vec::new(),
            heap: Vec::new(),
            records,
        }
    }

    pub fn run(&mut self) {
        while self.instruction_pointer < self.program.code.len() {
            let instruction = &self.program.code[self.instruction_pointer];
            self.instruction_pointer += 1;

            instruction.exec(self);
        }
    }

    pub fn result(&self) -> Option<Value> {
        match &self.program.return_ty {
            Ty::Field(field_ty) => match field_ty {
                FieldTy::IntI32 => {
                    let result = &self.stack[0..(IntBits::I32.bytes() as usize)];
                    let result = i32::from_be_bytes(result.try_into().unwrap());
                    Some(Value::Field(FieldValue::Int(result)))
                }
                FieldTy::Bool => {
                    let result = self.stack[0];
                    let result = if result == 0 { false } else { true };
                    Some(Value::Field(FieldValue::Bool(result)))
                }
                FieldTy::Timestamp => todo!(),
                FieldTy::Text => todo!(),
                FieldTy::RecordId { table_name } => {
                    let result = &self.stack[0..(IntBits::U128.bytes() as usize)];
                    let result = u128::from_be_bytes(result.try_into().unwrap());
                    Some(Value::Field(FieldValue::RecordId {
                        id: Ulid(result),
                        table_name: table_name.clone(),
                    }))
                }
            },
            Ty::Table(named) => None,
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
            Namespace::Record { idx } => panic!("cannot write to record memory"),
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
}
