use std::{
    borrow::Cow,
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

use bytepack::BytePacker;
use db_core::{
    asm_code::{AsmRuntime, compile_expr},
    defs::table::{TableData, TableDef, TableFieldDef},
    named::Named,
};

#[test]
fn expr_end2end() {
    let input = "-1 > -2 && (1 + 2 * 2) == 5";

    let expr = query_parse::parse_expr(input).unwrap();

    let program = compile_expr(&expr, &BTreeMap::new(), &HashSet::new()).unwrap();

    let mut query = ();
    let mut runtime = AsmRuntime::new(&program, Vec::new(), &mut query, []);
    runtime.run();
    let result = runtime.result_bool();

    assert!(result)
}

#[test]
fn expr_math() {
    let input = "1 + 1 + 1";

    let expr = query_parse::parse_expr(input).unwrap();

    let program = compile_expr(&expr, &BTreeMap::new(), &HashSet::new()).unwrap();

    let mut query = ();
    let mut runtime = AsmRuntime::new(&program, Vec::new(), &mut query, []);
    runtime.run();
    let result = runtime.result_i32();

    assert_eq!(result, 3)
}

#[test]
fn table_field_access() {
    let input = "person.age * 2 > 10 && person.has_pet && person.name == \"Billy\"";

    let expr = query_parse::parse_expr(input).unwrap();

    let table = TableDef {
        fields: vec![
            Named::new(
                "age",
                TableFieldDef {
                    ty: db_core::ty::FieldTy::IntI32,
                    has_index: false,
                },
            ),
            Named::new(
                "has_pet",
                TableFieldDef {
                    ty: db_core::ty::FieldTy::Bool,
                    has_index: false,
                },
            ),
            Named::new(
                "name",
                TableFieldDef {
                    ty: db_core::ty::FieldTy::Text,
                    has_index: false,
                },
            ),
        ],
        main_display_field: None,
    };

    let table = TableData::from(table);
    let record = BytePacker::create_fields(&table, |fields| {
        fields.pack("age", &26_i32);
        fields.pack("has_pet", &true);
        fields.pack("name", "Billy");
    });

    let tables = BTreeMap::from_iter([("person".into(), Arc::new(table))]);

    let program = compile_expr(&expr, &tables, &HashSet::new()).unwrap();

    let mut query = ();
    let mut runtime = AsmRuntime::new(&program, vec![Cow::Borrowed(&record)], &mut query, []);
    runtime.run();
    let result = runtime.result_bool();

    assert_eq!(result, true)
}
