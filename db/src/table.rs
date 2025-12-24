use std::{
    borrow::Cow,
    sync::{Arc, LazyLock},
};

use bytepack::{Pack, PackerField, PackerFormat, Unpack};

use crate::{Db, db::{IndexDef, IndexOnDelete}, field_type::FieldType};

pub struct NamedTable {
    pub name: Arc<str>,
    pub table: Table,
}

impl NamedTable {
    pub fn new(name: impl Into<Arc<str>>, table: Table) -> Self {
        Self {
            name: name.into(),
            table,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    fields: Vec<TableField>,
    main_display_field: Option<Arc<str>>,
}

impl Table {
    pub fn new(
        fields: Vec<TableField>,
        main_display_field: Option<Arc<str>>,
    ) -> Result<Self, TableCreateErr> {
        for field in &fields {
            let field_name_count = fields.iter().filter(|f| f.name() == field.name()).count();
            if field_name_count > 1 {
                return Err(TableCreateErr::DuplicateFieldName);
            }
        }

        if let Some(main_display_field) = &main_display_field {
            let field_name_count = fields
                .iter()
                .filter(|f| f.name() == main_display_field.as_ref())
                .count();

            if field_name_count != 1 {
                return Err(TableCreateErr::MainDisplayFieldNameInvalid);
            }
        }

        Ok(Self {
            fields,
            main_display_field,
        })
    }

    pub fn field(&self, name: &str) -> Option<&TableField> {
        self.fields
            .iter()
            .filter(|field| field.name() == name)
            .next()
    }

    pub fn fields(&self) -> &Vec<TableField> {
        &self.fields
    }

    pub fn main_display_field(&self) -> Option<&str> {
        self.main_display_field.as_ref().map(|f| f.as_ref())
    }

    pub fn packer_format(&self) -> PackerFormat {
        PackerFormat::new(self.fields.iter().map(|field| PackerField {
            name: field.name.clone(),
            size: field.ty.byte_count(),
        }))
    }
}

impl NamedTable {
    pub fn indices(&self) -> Vec<IndexDef> {
        let mut result = Vec::with_capacity(2);

        for field in &self.table.fields {
            match &field.ty {
                FieldType::Record { table_name } => {
                    let index_name = format!("#{}:{}", self.name, field.name);

                    result.push(IndexDef::new(index_name.into(), self.name.clone(), field.name.clone(), IndexOnDelete::Cascase));
                },
                _ => ()
            }
        }

        result
    }
}

#[derive(Debug)]
pub enum TableCreateErr {
    DuplicateFieldName,
    MainDisplayFieldNameInvalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableField {
    pub name: Arc<str>,
    pub ty: FieldType,
}

impl TableField {
    pub fn new(name: impl Into<Arc<str>>, ty: FieldType) -> Self {
        Self {
            name: name.into(),
            ty,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn ty(&self) -> &FieldType {
        &self.ty
    }
}

impl From<TableField> for PackerField {
    fn from(value: TableField) -> Self {
        PackerField {
            name: value.name,
            size: value.ty.byte_count(),
        }
    }
}

static TABLE_FIELD_FORMAT: LazyLock<PackerFormat> = LazyLock::new(|| {
    PackerFormat::new(
        [
            PackerField::new("name", Arc::<str>::PACK_BYTES),
            PackerField::new("ty", FieldType::PACK_BYTES),
        ]
        .into_iter(),
    )
});

impl Pack for TableField {
    const PACK_BYTES: u32 = Cow::<str>::PACK_BYTES + FieldType::PACK_BYTES;

    fn pack(&self, offset: u32, packer: &mut bytepack::BytePacker) {
        let mut packer = packer.fields(&TABLE_FIELD_FORMAT, offset);

        packer.pack("name", &self.name);
        packer.pack("ty", &self.ty);
    }
}

impl<'b> Unpack<'b> for TableField {
    fn unpack(offset: u32, unpacker: &bytepack::ByteUnpacker<'b>) -> Option<Self> {
        let unpacker = unpacker.fields(&TABLE_FIELD_FORMAT, offset);

        Some(Self {
            name: unpacker.unpack("name")?,
            ty: unpacker.unpack("ty")?,
        })
    }
}

static TABLE_FORMAT: LazyLock<PackerFormat> = LazyLock::new(|| {
    PackerFormat::new(
        [
            PackerField::new("fields", Vec::<TableField>::PACK_BYTES),
            PackerField::new("main_display_field", Option::<Arc<str>>::PACK_BYTES),
        ]
        .into_iter(),
    )
});

impl Pack for Table {
    const PACK_BYTES: u32 = Vec::<TableField>::PACK_BYTES + Option::<Arc<str>>::PACK_BYTES;

    fn pack(&self, offset: u32, packer: &mut bytepack::BytePacker) {
        let mut packer = packer.fields(&TABLE_FORMAT, offset);

        packer.pack("fields", &self.fields);
        packer.pack("main_display_field", &self.main_display_field);
    }
}

impl<'b> Unpack<'b> for Table {
    fn unpack(offset: u32, unpacker: &bytepack::ByteUnpacker<'b>) -> Option<Self> {
        let unpacker = unpacker.fields(&TABLE_FORMAT, offset);

        Some(Self {
            fields: unpacker.unpack("fields")?,
            main_display_field: unpacker.unpack("main_display_field")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use bytepack::{BytePacker, ByteUnpacker};

    use super::*;

    #[test]
    fn pack_unpack() {
        let table = Table::new(
            vec![TableField::new("name", FieldType::Text)],
            Some("name".into()),
        )
        .unwrap();

        let mut packer = BytePacker::new(Table::PACK_BYTES);
        table.pack(0, &mut packer);

        let bytes = packer.finish();

        let unpacker = ByteUnpacker::new(&bytes);

        let result = Table::unpack(0, &unpacker);

        assert_eq!(Some(table), result);
    }
}
