use core::slice;
use std::cmp::Ordering;

use chrono::{DateTime, Utc};
use ulid::Ulid;

use crate::{
    asm_code::{
        pointer::{AsmPointer, AsmSlicePointer},
        runtime::AsmRuntime,
    },
    expr::{CompareOp, EqOp, LogicOp, MathOp},
};

pub enum AsmCode<Pointer = AsmPointer> {
    TestInt {
        a: Pointer,
        b: Pointer,
        target: Pointer,
        bits: IntBits,
    },
    TestString {
        a: Pointer,
        b: Pointer,
        target: Pointer,
    },
    LogicOp {
        a: Pointer,
        b: Pointer,
        op: LogicOp,
        target: Pointer,
    },
    LogicNot {
        value: Pointer,
        target: Pointer,
    },
    MathOp {
        a: Pointer,
        b: Pointer,
        op: MathOp,
        target: Pointer,
        bits: IntBits,
    },
    NegateNum {
        value: Pointer,
        target: Pointer,
        bits: IntBits,
    },
    SetLiteral {
        target: Pointer,
        value: Literal,
    },
    SetLiteralConditional {
        test_result: Pointer,
        op: ConditionOp,
        target: Pointer,
        true_value: Literal,
        false_value: Option<Literal>,
    },
    Copy {
        src: Pointer,
        target: Pointer,
        len: u32,
    },
    ReserveStack {
        bytes: u32,
    },
    Jump {
        target: usize,
    },
    JumpConditional {
        test_result: Pointer,
        op: ConditionOp,
        target: usize,
    },
}

macro_rules! int_bit_match {
    ($bits:ident, ($($value:ident),*) => $expr:block) => {
        match $bits {
            IntBits::I8 => int_bit_match!(=> i8, $($value),*, $expr),
            IntBits::I16 => int_bit_match!(=> i16, $($value),*, $expr),
            IntBits::I32 => int_bit_match!(=> i32, $($value),*, $expr),
            IntBits::I64 => int_bit_match!(=> i64, $($value),*, $expr),
            IntBits::I128 => int_bit_match!(=> i128, $($value),*, $expr),
            IntBits::U8 => int_bit_match!(=> u8, $($value),*, $expr),
            IntBits::U16 => int_bit_match!(=> u16, $($value),*, $expr),
            IntBits::U32 => int_bit_match!(=> u32, $($value),*, $expr),
            IntBits::U64 => int_bit_match!(=> u64, $($value),*, $expr),
            IntBits::U128 => int_bit_match!(=> u128, $($value),*, $expr),
        }
    };
    ("signed" $bits:ident, ($($value:ident),*) => $expr:block else $default:expr) => {
        match $bits {
            IntBits::I8 => int_bit_match!(=> i8, $($value),*, $expr),
            IntBits::I16 => int_bit_match!(=> i16, $($value),*, $expr),
            IntBits::I32 => int_bit_match!(=> i32, $($value),*, $expr),
            IntBits::I64 => int_bit_match!(=> i64, $($value),*, $expr),
            IntBits::I128 => int_bit_match!(=> i128, $($value),*, $expr),
            _ => $default
        }
    };
    (=> $ty:ty, $($value:ident),*, $expr:block) => {{
        $(let $value = <$ty>::from_be_bytes($value.try_into().unwrap());)*

        $expr
    }};
}

impl AsmCode {
    pub fn exec(&self, runtime: &mut AsmRuntime) {
        match self {
            AsmCode::TestInt { a, b, target, bits } => {
                let bytes = bits.bytes();

                let a = runtime.get(a, bytes);
                let b = runtime.get(b, bytes);

                let result = int_bit_match!(bits, (a, b) => { a.cmp(&b) });

                let result_value = Self::ordering_byte(result);

                runtime.set(target, &[result_value])
            }
            AsmCode::TestString { a, b, target } => {
                let a_ptr = runtime.get(a, AsmSlicePointer::BYTES);
                let a = AsmSlicePointer::from_bytes(a_ptr.try_into().unwrap());

                let b_ptr = runtime.get(b, AsmSlicePointer::BYTES);
                let b = AsmSlicePointer::from_bytes(b_ptr.try_into().unwrap());

                let a = runtime.get(&a.pointer, a.len);
                let b = runtime.get(&b.pointer, b.len);

                let a = str::from_utf8(a).unwrap();
                let b = str::from_utf8(b).unwrap();

                let result_value = Self::ordering_byte(a.cmp(b));

                runtime.set(target, &[result_value])
            }
            AsmCode::LogicOp { a, b, op, target } => {
                let [a]: [u8; 1] = runtime.get(a, 1).try_into().unwrap();
                let [b]: [u8; 1] = runtime.get(b, 1).try_into().unwrap();

                let a = match a {
                    0 => false,
                    1 => true,
                    _ => panic!(),
                };

                let b = match b {
                    0 => false,
                    1 => true,
                    _ => panic!(),
                };

                let result = match op {
                    LogicOp::And => a && b,
                    LogicOp::Or => a || b,
                };

                let result = match result {
                    true => 1,
                    false => 0,
                };

                runtime.set(target, &[result]);
            }
            AsmCode::LogicNot { value, target } => {
                let [value]: [u8; 1] = runtime.get(value, 1).try_into().unwrap();

                let value = match value {
                    0 => false,
                    1 => true,
                    _ => panic!(),
                };

                let result = !value;

                let result = match result {
                    true => 1,
                    false => 0,
                };

                runtime.set(target, &[result]);
            }
            AsmCode::MathOp {
                a,
                b,
                op,
                target,
                bits,
            } => {
                let bytes = bits.bytes();

                let a = runtime.get(a, bytes);
                let b = runtime.get(b, bytes);

                let result: Literal = int_bit_match!(bits, (a, b) => { match op {
                    MathOp::Add => (a + b).into(),
                    MathOp::Sub => (a - b).into(),
                    MathOp::Mul => (a * b).into(),
                    MathOp::Div => (a / b).into(),
                } });

                runtime.set(target, result.as_ref());
            }
            AsmCode::NegateNum {
                value,
                target,
                bits,
            } => {
                let bytes = bits.bytes();

                let value = runtime.get(value, bytes);

                let result: Literal =
                    int_bit_match!("signed" bits, (value) => { (-value).into() } else panic!());

                runtime.set(target, result.as_ref());
            }
            AsmCode::SetLiteral { target, value } => {
                runtime.set(target, value.as_ref());
            }
            AsmCode::Copy { src, target, len } => {
                let value = runtime.get(src, *len).to_owned(); // TODO: add runtime.copy(...)
                runtime.set(target, &value);
            }
            AsmCode::SetLiteralConditional {
                test_result,
                op,
                target,
                true_value,
                false_value,
            } => {
                if Self::passes_test_result(runtime, test_result, op) {
                    runtime.set(target, true_value.as_ref());
                } else if let Some(false_value) = false_value {
                    runtime.set(target, false_value.as_ref());
                }
            }
            AsmCode::ReserveStack { bytes } => runtime.reserve_stack(*bytes),
            AsmCode::Jump { target } => runtime.jump(*target),
            AsmCode::JumpConditional {
                test_result,
                op,
                target,
            } => {
                if Self::passes_test_result(runtime, test_result, op) {
                    runtime.jump(*target);
                }
            }
        }
    }

    fn ordering_byte(ordering: Ordering) -> u8 {
        match ordering {
            std::cmp::Ordering::Less => 'l' as u8,
            std::cmp::Ordering::Equal => 'e' as u8,
            std::cmp::Ordering::Greater => 'g' as u8,
        }
    }

    fn passes_test_result(
        runtime: &AsmRuntime,
        test_result: &AsmPointer,
        op: &ConditionOp,
    ) -> bool {
        let [result]: [u8; 1] = runtime.get(test_result, 1).try_into().unwrap();

        match (op, result as char) {
            (ConditionOp::Compare(CompareOp::Greater), 'g') => true,
            (ConditionOp::Compare(CompareOp::GreaterEq), 'g' | 'e') => true,
            (ConditionOp::Compare(CompareOp::Less), 'l') => true,
            (ConditionOp::Compare(CompareOp::LessEq), 'l' | 'e') => true,
            (ConditionOp::Eq(EqOp::Eq), 'e') => true,
            (ConditionOp::Eq(EqOp::Neq), 'l' | 'g') => true,
            _ => false,
        }
    }
}

pub enum IntBits {
    I8,
    I16,
    I32,
    I64,
    I128,
    U8,
    U16,
    U32,
    U64,
    U128,
}

impl IntBits {
    pub fn bytes(&self) -> u32 {
        match self {
            IntBits::I8 | IntBits::U8 => 1,
            IntBits::I16 | IntBits::U16 => 2,
            IntBits::I32 | IntBits::U32 => 4,
            IntBits::I64 | IntBits::U64 => 8,
            IntBits::I128 | IntBits::U128 => 16,
        }
    }
}

pub enum ConditionOp {
    Compare(CompareOp),
    Eq(EqOp),
}

impl ConditionOp {
    pub fn negate(self) -> Self {
        match self {
            ConditionOp::Compare(CompareOp::Greater) => ConditionOp::Compare(CompareOp::LessEq),
            ConditionOp::Compare(CompareOp::GreaterEq) => ConditionOp::Compare(CompareOp::Less),
            ConditionOp::Compare(CompareOp::Less) => ConditionOp::Compare(CompareOp::GreaterEq),
            ConditionOp::Compare(CompareOp::LessEq) => ConditionOp::Compare(CompareOp::Greater),
            ConditionOp::Eq(EqOp::Eq) => ConditionOp::Eq(EqOp::Neq),
            ConditionOp::Eq(EqOp::Neq) => ConditionOp::Eq(EqOp::Eq),
        }
    }
}

pub enum Literal {
    B8([u8; 1]),
    B16([u8; 2]),
    B32([u8; 4]),
    B64([u8; 8]),
    B96([u8; 12]),
    B128([u8; 16]),
}

impl AsRef<[u8]> for Literal {
    fn as_ref(&self) -> &[u8] {
        match self {
            Literal::B8(v) => v,
            Literal::B16(v) => v,
            Literal::B32(v) => v,
            Literal::B64(v) => v,
            Literal::B96(v) => v,
            Literal::B128(v) => v,
        }
    }
}

macro_rules! impl_literal {
    ($ty:ty, $variant:ident) => {
        impl From<$ty> for Literal {
            fn from(value: $ty) -> Self {
                Self::$variant(value.to_be_bytes())
            }
        }
    };
}

impl_literal!(u8, B8);
impl_literal!(u16, B16);
impl_literal!(u32, B32);
impl_literal!(u64, B64);
impl_literal!(u128, B128);
impl_literal!(i8, B8);
impl_literal!(i16, B16);
impl_literal!(i32, B32);
impl_literal!(i64, B64);
impl_literal!(i128, B128);

impl_literal!(usize, B64);
impl_literal!(isize, B64);

impl From<bool> for Literal {
    fn from(value: bool) -> Self {
        Self::B8(if value { [1] } else { [0] })
    }
}

impl From<DateTime<Utc>> for Literal {
    fn from(value: DateTime<Utc>) -> Self {
        let timestamp = value.timestamp();
        timestamp.into()
    }
}

impl From<Ulid> for Literal {
    fn from(value: Ulid) -> Self {
        value.0.into()
    }
}
