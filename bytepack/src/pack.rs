use std::{borrow::Cow, sync::Arc};

use chrono::{Timelike, Utc};
use ulid::Ulid;

use crate::{BytePacker, ByteUnpacker, PackPointer};

pub trait Pack {
    const PACK_BYTES: u32;
    fn pack(&self, offset: u32, packer: &mut BytePacker);
}

pub trait Unpack<'b>: Sized {
    fn unpack(offset: u32, unpacker: &ByteUnpacker<'b>) -> Option<Self>;
}

// ---------- primitive types ----------

macro_rules! num_pack {
    ($ty:ty) => {
        impl Pack for $ty {
            const PACK_BYTES: u32 = Self::BITS / 8;

            fn pack(&self, offset: u32, packer: &mut BytePacker) {
                packer.pack_bytes(offset, self.to_be_bytes().as_ref());
            }
        }

        impl<'b> Unpack<'b> for $ty {
            fn unpack(offset: u32, unpacker: &ByteUnpacker<'b>) -> Option<Self> {
                let bytes = unpacker.read_bytes(PackPointer {
                    offset,
                    len: Self::PACK_BYTES,
                });

                let bytes = bytes.try_into().unwrap();

                Some(Self::from_be_bytes(bytes))
            }
        }
    };
}

num_pack!(u8);
num_pack!(u16);
num_pack!(u32);
num_pack!(u64);
num_pack!(u128);

num_pack!(i8);
num_pack!(i16);
num_pack!(i32);
num_pack!(i64);
num_pack!(i128);

impl Pack for bool {
    const PACK_BYTES: u32 = 1;

    fn pack(&self, offset: u32, packer: &mut BytePacker) {
        let v = if *self { &[1] } else { &[0] };

        packer.pack_bytes(offset, v.as_ref());
    }
}

impl<'b> Unpack<'b> for bool {
    fn unpack(offset: u32, unpacker: &ByteUnpacker<'b>) -> Option<Self> {
        let bytes = unpacker.read_bytes(PackPointer {
            offset,
            len: Self::PACK_BYTES,
        });

        let bytes: [u8; 1] = bytes.try_into().unwrap();

        match bytes {
            [1] => Some(true),
            [0] => Some(false),
            _ => None,
        }
    }
}

// ---------- string ----------

impl Pack for String {
    const PACK_BYTES: u32 = PackPointer::PACK_BYTES;

    fn pack(&self, offset: u32, packer: &mut BytePacker) {
        packer.pack_bytes_indirect(offset, self.as_bytes());
    }
}

impl<'b> Unpack<'b> for String {
    fn unpack(offset: u32, unpacker: &ByteUnpacker<'b>) -> Option<Self> {
        let bytes = unpacker.read_indirect(offset)?;

        Self::from_utf8(bytes.to_owned()).ok()
    }
}

impl Pack for str {
    const PACK_BYTES: u32 = PackPointer::PACK_BYTES;

    fn pack(&self, offset: u32, packer: &mut BytePacker) {
        packer.pack_bytes_indirect(offset, self.as_bytes());
    }
}

impl<'b> Unpack<'b> for &'b str {
    fn unpack(offset: u32, unpacker: &ByteUnpacker<'b>) -> Option<Self> {
        let bytes = unpacker.read_indirect(offset)?;

        str::from_utf8(bytes).ok()
    }
}

impl Pack for Arc<str> {
    const PACK_BYTES: u32 = str::PACK_BYTES;

    fn pack(&self, offset: u32, packer: &mut BytePacker) {
        self.as_ref().pack(offset, packer);
    }
}

impl<'b> Unpack<'b> for Arc<str> {
    fn unpack(offset: u32, unpacker: &ByteUnpacker<'b>) -> Option<Self> {
        Some(<&str>::unpack(offset, unpacker)?.into())
    }
}

// ---------- bytes ----------

impl Pack for [u8] {
    const PACK_BYTES: u32 = PackPointer::PACK_BYTES;

    fn pack(&self, offset: u32, packer: &mut BytePacker) {
        packer.pack_bytes_indirect(offset, self);
    }
}

impl<'b> Unpack<'b> for &'b [u8] {
    fn unpack(offset: u32, unpacker: &ByteUnpacker<'b>) -> Option<Self> {
        unpacker.read_indirect(offset)
    }
}

// ---------- generics ----------

impl<T: Pack> Pack for Option<T> {
    const PACK_BYTES: u32 = PackPointer::PACK_BYTES;

    fn pack(&self, offset: u32, packer: &mut BytePacker) {
        match self {
            Some(value) => {
                packer.pack_indirect(offset, value);
            }
            None => {
                PackPointer::NULL.pack(offset, packer);
            }
        }
    }
}

impl<'b, T: Unpack<'b>> Unpack<'b> for Option<T> {
    fn unpack(offset: u32, unpacker: &ByteUnpacker<'b>) -> Option<Self> {
        let ptr = PackPointer::unpack(offset, unpacker)?;

        if ptr == PackPointer::NULL {
            Some(None)
        } else {
            let value = T::unpack(ptr.offset, unpacker)?;

            Some(Some(value))
        }
    }
}

impl<'a, T: Pack + ToOwned + ?Sized> Pack for Cow<'a, T> {
    const PACK_BYTES: u32 = T::PACK_BYTES;

    fn pack(&self, offset: u32, packer: &mut BytePacker) {
        self.as_ref().pack(offset, packer);
    }
}

impl<'b, T: ToOwned + ?Sized> Unpack<'b> for Cow<'b, T>
where
    &'b T: Unpack<'b>,
{
    fn unpack(offset: u32, unpacker: &ByteUnpacker<'b>) -> Option<Self> {
        let v = <&T>::unpack(offset, unpacker)?;

        Some(Cow::Borrowed(v))
    }
}

impl<T: Pack> Pack for Vec<T> {
    const PACK_BYTES: u32 = PackPointer::PACK_BYTES;

    fn pack(&self, offset: u32, packer: &mut BytePacker) {
        let needed_bytes = T::PACK_BYTES as usize * self.len();

        let ptr = packer.push_dynamic(vec![0; needed_bytes].as_ref());
        ptr.pack(offset, packer);

        for (i, v) in self.iter().enumerate() {
            let offset = ptr.offset + i as u32 * T::PACK_BYTES;

            v.pack(offset, packer);
        }
    }
}

impl<'b, T: Pack + Unpack<'b>> Unpack<'b> for Vec<T> {
    fn unpack(offset: u32, unpacker: &ByteUnpacker<'b>) -> Option<Self> {
        let ptr = PackPointer::unpack(offset, unpacker)?;

        let count = ptr.len / T::PACK_BYTES;

        let mut values = Vec::with_capacity(count as usize);

        for i in 0..count {
            let offset = ptr.offset + i * T::PACK_BYTES;

            values.push(T::unpack(offset, unpacker)?);
        }

        Some(values)
    }
}

// ---------- extern types ----------
// ----------     ulid     ----------

impl Pack for Ulid {
    const PACK_BYTES: u32 = u128::PACK_BYTES;

    fn pack(&self, offset: u32, packer: &mut BytePacker) {
        self.0.pack(offset, packer);
    }
}

impl<'b> Unpack<'b> for Ulid {
    fn unpack(offset: u32, unpacker: &ByteUnpacker<'b>) -> Option<Self> {
        Some(Ulid(u128::unpack(offset, unpacker)?))
    }
}

// ---------- extern types ----------
// ----------    chrono    ----------

impl Pack for chrono::DateTime<Utc> {
    const PACK_BYTES: u32 = i64::PACK_BYTES;

    fn pack(&self, offset: u32, packer: &mut BytePacker) {
        self.timestamp().pack(offset, packer);
    }
}

impl<'b> Unpack<'b> for chrono::DateTime<Utc> {
    fn unpack(offset: u32, unpacker: &ByteUnpacker<'b>) -> Option<Self> {
        let timestamp = i64::unpack(offset, unpacker)?;

        Self::from_timestamp(timestamp, 0)
    }
}

impl Pack for chrono::NaiveDate {
    const PACK_BYTES: u32 = i32::PACK_BYTES;

    fn pack(&self, offset: u32, packer: &mut BytePacker) {
        self.to_epoch_days().pack(offset, packer);
    }
}

impl<'b> Unpack<'b> for chrono::NaiveDate {
    fn unpack(offset: u32, unpacker: &ByteUnpacker<'b>) -> Option<Self> {
        let timestamp = i32::unpack(offset, unpacker)?;

        Self::from_epoch_days(timestamp)
    }
}
impl Pack for chrono::NaiveTime {
    const PACK_BYTES: u32 = u32::PACK_BYTES;

    fn pack(&self, offset: u32, packer: &mut BytePacker) {
        self.num_seconds_from_midnight().pack(offset, packer);
    }
}

impl<'b> Unpack<'b> for chrono::NaiveTime {
    fn unpack(offset: u32, unpacker: &ByteUnpacker<'b>) -> Option<Self> {
        let timestamp = u32::unpack(offset, unpacker)?;

        Self::from_num_seconds_from_midnight_opt(timestamp, 0)
    }
}
