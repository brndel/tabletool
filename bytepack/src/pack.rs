use std::{borrow::Cow, collections::BTreeMap, sync::Arc};

use chrono::{Timelike, Utc};
use ulid::Ulid;

use crate::{BytePacker, ByteUnpacker, PackPointer};

pub use bytepack_derive::{Pack, Unpack};

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

// ---------- refs & tuples ----------

impl<T: Pack> Pack for &T {
    const PACK_BYTES: u32 = T::PACK_BYTES;

    fn pack(&self, offset: u32, packer: &mut BytePacker) {
        T::pack(&self, offset, packer);
    }
}

impl<T: Pack> Pack for &mut T {
    const PACK_BYTES: u32 = T::PACK_BYTES;

    fn pack(&self, offset: u32, packer: &mut BytePacker) {
        T::pack(&self, offset, packer);
    }
}

impl<T1: Pack, T2: Pack> Pack for (T1, T2) {
    const PACK_BYTES: u32 = T1::PACK_BYTES + T2::PACK_BYTES;

    fn pack(&self, offset: u32, packer: &mut BytePacker) {
        let (v1, v2) = self;

        v1.pack(offset, packer);
        v2.pack(offset + T1::PACK_BYTES, packer);
    }
}

impl<'b, T1: Unpack<'b> + Pack, T2: Unpack<'b>> Unpack<'b> for (T1, T2) {
    fn unpack(offset: u32, unpacker: &ByteUnpacker<'b>) -> Option<Self> {
        let v1 = T1::unpack(offset, unpacker)?;
        let v2 = T2::unpack(offset + T1::PACK_BYTES, unpacker)?;

        Some((v1, v2))
    }
}

// ---------- collections ----------

impl<T: Pack> Pack for Vec<T> {
    const PACK_BYTES: u32 = PackPointer::PACK_BYTES;

    fn pack(&self, offset: u32, packer: &mut BytePacker) {
        pack_iter(offset, packer, self.iter());
    }
}

impl<'b, T: Pack + Unpack<'b>> Unpack<'b> for Vec<T> {
    fn unpack(offset: u32, unpacker: &ByteUnpacker<'b>) -> Option<Self> {
        let iter = unpack_iter(offset, unpacker)?;

        iter.collect()
    }
}

impl<K: Pack, V: Pack> Pack for BTreeMap<K, V> {
    const PACK_BYTES: u32 = PackPointer::PACK_BYTES;

    fn pack(&self, offset: u32, packer: &mut BytePacker) {
        pack_iter(offset, packer, self.iter());
    }
}

impl<'b, K: Unpack<'b> + Pack + Ord, V: Unpack<'b> + Pack> Unpack<'b> for BTreeMap<K, V> {
    fn unpack(offset: u32, unpacker: &ByteUnpacker<'b>) -> Option<Self> {
        let iter = unpack_iter(offset, unpacker)?;

        iter.collect()
    }
}

fn pack_iter<T: Pack>(
    offset: u32,
    packer: &mut BytePacker,
    iter: impl ExactSizeIterator<Item = T>,
) {
    let needed_bytes = T::PACK_BYTES as usize * iter.len();

    let ptr = packer.push_dynamic(vec![0; needed_bytes].as_ref());
    ptr.pack(offset, packer);

    for (i, v) in iter.enumerate() {
        let offset = ptr.offset + i as u32 * T::PACK_BYTES;

        v.pack(offset, packer);
    }
}

fn unpack_iter<'b, T: Pack + Unpack<'b>>(
    offset: u32,
    unpacker: &ByteUnpacker<'b>,
) -> Option<impl ExactSizeIterator<Item = Option<T>>> {
    let ptr = PackPointer::unpack(offset, unpacker)?;

    let count = ptr.len / T::PACK_BYTES;

    Some((0..count).map(move |i| {
        let offset = ptr.offset + i * T::PACK_BYTES;

        T::unpack(offset, unpacker)
    }))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Pack, Unpack, PartialEq, Debug)]
    struct PackDerive {
        a: u8,
        b: Vec<String>,
    }

    #[derive(Pack, Unpack)]
    #[allow(unused)]
    struct GenericDerive<T> {
        header: u8,
        value: T
    }

    #[test]
    fn test_name() {
        assert_eq!(
            PackDerive::PACK_BYTES,
            u8::PACK_BYTES + Vec::<String>::PACK_BYTES
        );

        let value = vec![
            PackDerive {
                a: 123,
                b: vec!["hello".to_owned(), "world".to_owned()],
            },
            PackDerive {
                a: 187,
                b: vec!["foo".to_owned(), "bar".to_owned()],
            },
        ];

        let bytes = BytePacker::pack_value(&value);
        let result = Vec::<PackDerive>::unpack(0, &ByteUnpacker::new(&bytes));

        assert_eq!(Some(value), result);
    }
}
