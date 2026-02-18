use std::{cell::LazyCell, sync::Arc};

use bytepack::{Pack, PackField, PackFormat, Unpack};

use crate::{defs::index::IndexDef, named::Named, ty::FieldTy};

#[derive(Debug, PartialEq, Eq, Clone, Pack, Unpack)]
pub struct TableDef {
    pub fields: Vec<Named<TableFieldDef>>,
    pub main_display_field: Option<u32>,
}

#[derive(Debug, PartialEq, Eq, Clone, Pack, Unpack)]
pub struct TableFieldDef {
    pub ty: FieldTy,
    pub has_index: bool,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct TableData {
    fields: Vec<Named<TableFieldData>>,
    index: Vec<(Arc<str>, usize)>,
    main_display_field: Option<usize>,
    fixed_byte_count: u32,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct TableFieldData {
    pub ty: FieldTy,
    pub offset: u32,
    pub has_index: bool,
}

impl From<TableDef> for TableData {
    fn from(value: TableDef) -> Self {
        let mut offset = 0;

        let fields = value.fields.into_iter().map(|field| {
            let Named { name, value: field } = field;
            let field = TableFieldData {
                offset,
                ty: field.ty,
                has_index: field.has_index,
            };

            offset += field.ty.byte_count();

            Named::new(name, field)
        });

        let fields_vec = fields.collect::<Vec<_>>();

        let index = {
            let mut vec = fields_vec
                .iter()
                .enumerate()
                .map(|(idx, field)| (field.name.clone(), idx))
                .collect::<Vec<_>>();

            vec.sort_by(|(a, _), (b, _)| a.cmp(b));

            vec
        };

        let main_display_field = value.main_display_field.and_then(|idx| {
            let idx = idx as usize;

            (idx < fields_vec.len()).then_some(idx)
        });

        Self {
            fields: fields_vec,
            index,
            main_display_field,
            fixed_byte_count: offset,
        }
    }
}

impl TableData {
    pub fn indices(&self, table_name: &Arc<str>) -> Vec<IndexDef> {
        let mut result = Vec::with_capacity(2);

        for Named {
            name: field_name,
            value: field,
        } in &self.fields
        {
            let index_name =
                LazyCell::new(|| Arc::<str>::from(format!("#{}:{}", table_name, field_name)));

            match field.ty {
                FieldTy::RecordId { .. } => {
                    result.push(IndexDef {
                        index_name: index_name.clone(),
                        table_name: table_name.clone(),
                        field_name: field_name.clone(),
                        on_delete: crate::defs::index::IndexOnDelete::Cascase,
                    });
                    continue;
                }
                _ => (),
            }

            if field.has_index {
                result.push(IndexDef {
                    index_name: index_name.clone(),
                    table_name: table_name.clone(),
                    field_name: field_name.clone(),
                    on_delete: super::index::IndexOnDelete::None,
                });
            }
        }

        result
    }

    pub fn main_display_field(&self) -> Option<&Named<TableFieldData>> {
        match self.main_display_field {
            Some(idx) => Some(&self.fields[idx]),
            None => None,
        }
    }

    pub fn main_display_field_idx(&self) -> Option<usize> {
        self.main_display_field
    }

    pub fn fields(&self) -> impl Iterator<Item = &Named<TableFieldData>> {
        self.fields.iter()
    }

    pub fn has_field(&self, name: &str) -> bool {
        self.index_of_field(name).is_some()
    }
}

impl TableData {
    fn index_of_field(&self, name: &str) -> Option<usize> {
        match self
            .index
            .binary_search_by_key(&name, |(name, _)| name.as_ref())
        {
            Ok(index_idx) => {
                let (_, field_idx) = self.index[index_idx];
                Some(field_idx)
            }
            Err(_) => None,
        }
    }
}

impl PackFormat for TableData {
    type Field = TableFieldData;

    fn field<'a>(&'a self, name: &str) -> Option<&'a Self::Field> {
        match self.index_of_field(name) {
            Some(idx) => {
                Some(&self.fields[idx].value)
            },
            None => None,
        }
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
