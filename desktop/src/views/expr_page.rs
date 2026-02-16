use chrono::Utc;
use db::{Db, DbError};
use db_core::{
    defs::table::TableData,
    expr::EvalCtx,
    query::Query,
    record::RecordBytes,
};
use dioxus::prelude::*;
use query_parse::parse_expr;
use ui::DataTable;

#[component]
pub fn ExprPage() -> Element {
    let db = use_context::<Db>();

    let mut text_value = use_signal(|| String::new());

    let expr = text_value.with(|value| parse_expr(value));

    let expr_result = expr.as_ref().map(|expr| {
        expr.eval(&EvalCtx {
            now: Utc::now(),
            ..Default::default()
        })
    });

    let query = use_memo(move || {
        println!("parsing query");
        query_parse::parse(&text_value.read())
    });

    let mut query_result = use_store({
        let db = db.clone();
        move || {
            println!("creating store");
            Some(QueryResult::new(query()?, &db))
        }
    });

    use_effect(move || {
        if let Some(query) = query() {
            println!("updating store");
            query_result.set(Some(QueryResult::new(query, &db)));
        }
    });

    rsx! {
        textarea {
            class: "query-input",
            placeholder: "Query/Expr",
            spellcheck: false,
            autocomplete: false,
            oninput: move |e| text_value.set(e.value()),
        }
        div {
            "Expr: {expr:?}",
        }
        div {
            "Result: {expr_result:?}",
        }
        div {
            "Query: {query:?}",
        }
        if let Some(result) = query_result.transpose() && let Ok(items) = result.result().transpose() && let Some(table) = result.result_table().transpose() {
            DataTable {
                items: items,
                delete: |_| (),
                table: table,
                table_name: "Foobar",
            }
        }
        // div {
        //     "Result: {query_result:?}",
        // }
    }
}

#[derive(Store)]
struct QueryResult {
    query: Query,
    result: Result<Vec<RecordBytes>, DbError>,
    result_table: Option<TableData>,
}

impl QueryResult {
    pub fn new(query: Query, db: &Db) -> Self {
        let result = db.run_query(&query);
        let result_table = db.table(&query.table_name);

        Self {
            query,
            result,
            result_table,
        }
    }
}
