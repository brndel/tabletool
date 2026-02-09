use std::{
    collections::BTreeMap,
    sync::{Arc, LazyLock},
};

use bytepack::{HasPackerFormat, Pack, PackField, PackFormat, PackerField, PackerFormat, Unpack};

use crate::{defs::index::IndexDef, ty::FieldTy};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct TableDef {
    pub fields: BTreeMap<Arc<str>, TableFieldDef>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct TableFieldDef {
    pub ty: FieldTy,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct TableData {
    pub fields: BTreeMap<Arc<str>, TableFieldData>,
    pub fixed_byte_count: u32,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct TableFieldData {
    pub ty: FieldTy,
    pub offset: u32,
}

impl From<TableDef> for TableData {
    fn from(value: TableDef) -> Self {
        let mut offset = 0;

        let fields = value.fields.into_iter().map(|(name, field)| {
            let field = TableFieldData {
                offset,
                ty: field.ty,
            };

            offset += field.ty.byte_count();

            (name, field)
        });

        Self {
            fields: fields.collect(),
            fixed_byte_count: offset,
        }
    }
}

impl TableData {
    pub fn indices(&self, table_name: &Arc<str>) -> Vec<IndexDef> {
        let mut result = Vec::with_capacity(2);

        for (field_name, field) in &self.fields {
            let index_name = format!("#{}:{}", table_name, field_name);

            match field.ty {
                FieldTy::RecordId { .. } => {
                    result.push(IndexDef {
                        index_name: index_name.into(),
                        table_name: table_name.clone(),
                        field_name: field_name.clone(),
                        on_delete: crate::defs::index::IndexOnDelete::Cascase,
                    });
                    continue;
                }
                _ => (),
            }

            // if field.has_index {
            //     result.push(IndexDef::new(
            //         index_name.into(),
            //         self.name.clone(),
            //         field.name.clone(),
            //         IndexOnDelete::None,
            //     ));
            // }
        }

        result
    }
}

impl PackFormat for TableData {
    type Field = TableFieldData;

    fn field<'a>(&'a self, name: &str) -> Option<&'a Self::Field> {
        self.fields.get(name)
    }

    fn fixed_byte_count(&self) -> u32 {
        self.fixed_byte_count
    }
}

impl PackField for TableFieldData {
    fn offset(&self) -> u32 {
        self.offset
    }
}

impl HasPackerFormat for TableFieldDef {
    fn packer_format() -> &'static bytepack::PackerFormat {
        static FORMAT: LazyLock<PackerFormat> =
            LazyLock::new(|| PackerFormat::new([PackerField::new("ty", FieldTy::PACK_BYTES)]));

        &FORMAT
    }
}

impl Pack for TableFieldDef {
    const PACK_BYTES: u32 = FieldTy::PACK_BYTES;

    fn pack(&self, offset: u32, packer: &mut bytepack::BytePacker) {
        let mut packer = packer.fields(Self::packer_format(), offset);

        packer.pack("ty", &self.ty);
    }
}

impl<'b> Unpack<'b> for TableFieldDef {
    fn unpack(offset: u32, unpacker: &bytepack::ByteUnpacker<'b>) -> Option<Self> {
        let unpacker = unpacker.fields(Self::packer_format(), offset);

        Some(Self {
            ty: unpacker.unpack("ty")?,
        })
    }
}

impl Pack for TableDef {
    const PACK_BYTES: u32 = BTreeMap::<Arc<str>, TableFieldDef>::PACK_BYTES;

    fn pack(&self, offset: u32, packer: &mut bytepack::BytePacker) {
        self.fields.pack(offset, packer);
    }
}

impl<'b> Unpack<'b> for TableDef {
    fn unpack(offset: u32, unpacker: &bytepack::ByteUnpacker<'b>) -> Option<Self> {
        Some(Self {
            fields: Unpack::unpack(offset, unpacker)?,
        })
    }
}
