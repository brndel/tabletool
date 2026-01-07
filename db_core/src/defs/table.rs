use std::{
    collections::BTreeMap,
    sync::{Arc, LazyLock},
};

use bytepack::{HasPackerFormat, Pack, PackField, PackFormat, PackerField, PackerFormat};

use crate::ty::FieldTy;

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
