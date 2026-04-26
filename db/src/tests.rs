use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap, HashSet},
    pin::Pin,
    sync::Arc,
};

use bytepack::BytePacker;
use db_core::{
    compile::CompilerDiagnostics,
    defs::table::{TableData, TableDef, TableFieldDef},
    eval_lang::{
        compile::compile,
        runtime::{EvalRuntime, RecordProvider},
    },
    named::Named,
    value::Value,
};
use ulid::Ulid;

#[derive(Clone)]
struct HardcodedRecordProvider {
    records: HashMap<String, HashMap<Ulid, Vec<u8>>>,
}

impl RecordProvider for HardcodedRecordProvider {
    fn fetch_record(&self, table_name: &str, record: ulid::Ulid) -> Option<Cow<'_, [u8]>> {
        Some(self.records.get(table_name)?.get(&record)?.into())
    }

    fn iter_table(&self, table_name: &str, mut f: impl FnMut(Ulid, Cow<'_, [u8]>)) {
        for (id, data) in self.records.get(table_name).unwrap().iter() {
            f(*id, Cow::Borrowed(data))
        }
    }
}

#[test]
fn expr_query() {
    // let input = "-1 > -2 && (1 + 2 * 2) == 5";
    let input = "query project where |project| => {true}";
    // let input = "sum([1, 2, 3, 10])";

    run_expr_in_ctx(input);
}

#[test]
fn iter_sum() {
    let result = run_expr_in_ctx("sum([1, 2, 3, 10])");

    assert_eq!(result, Value::Field(db_core::value::FieldValue::Int(16)));
}

#[test]
fn block_expr() {
    let result = run_expr_in_ctx(
        "{
        let x = {
            let x = \"hello world\";
            let y = \"🫩\";
            x.ptr() 
        };

        let y = {
            let y = 10;
            y
        };

        x * y
    }",
    );

    assert_eq!(result, Value::Field(db_core::value::FieldValue::Int(160)));
}

fn run_expr_in_ctx(input: &str) -> Value {
    let (expr, errors) = query_parse::parse_expr(input);

    for err in errors {
        println!("ERR: {err:?}");
    }

    let expr = expr.unwrap();

    let project_table = TableDef {
        fields: vec![
            Named::new(
                "is_fun",
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
            Named::new(
                "prio",
                TableFieldDef {
                    ty: db_core::ty::FieldTy::IntI32,
                    has_index: false,
                },
            ),
            Named::new(
                "group",
                TableFieldDef {
                    ty: db_core::ty::FieldTy::RecordId {
                        table_name: "group".into(),
                    },
                    has_index: false,
                },
            ),
        ],
        main_display_field: None,
    };

    let group_table = TableDef {
        fields: vec![Named::new(
            "name",
            TableFieldDef {
                ty: db_core::ty::FieldTy::Text,
                has_index: false,
            },
        )],
        main_display_field: None,
    };

    let group_table = TableData::from(group_table);
    let group_id = Ulid::from_string("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    let group_record = BytePacker::create_fields(&group_table, |fields| {
        fields.pack("name", "Privater Kram");
    });

    let project_table = TableData::from(project_table);
    let project_id = Ulid::from_string("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    let project_record = BytePacker::create_fields(&project_table, |fields| {
        fields.pack("name", "TableTool");
        fields.pack("is_func", &true);
        fields.pack("prio", &10_i32);
        fields.pack("group", &group_id);
    });

    let tables = BTreeMap::from_iter([
        ("project".into(), Arc::new(project_table)),
        ("group".into(), Arc::new(group_table)),
    ]);

    let mut store = wasmer::Store::new(wasmer::Engine::default());

    let mut diagnostics = CompilerDiagnostics::new();
    let Some(program) = compile(&expr.value, &mut diagnostics, &tables) else {
        dbg!(diagnostics);
        panic!("program did not compile");
    };

    let compiled_program = program.compile(&store).unwrap();

    let runtime = EvalRuntime::new(HardcodedRecordProvider {
        records: HashMap::from_iter([
            (
                "project".into(),
                HashMap::from_iter([(project_id, project_record)]),
            ),
            (
                "group".into(),
                HashMap::from_iter([(group_id, group_record)]),
            ),
        ]),
    });

    let result = runtime.run_program(&mut store, &compiled_program);

    dbg!(result)
}
