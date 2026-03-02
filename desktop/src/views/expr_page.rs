use db::{Db, DbError};
use db_core::query::{
    Query, QueryResult, QueryResultGroupStoreExt, QueryResultStoreExt, QueryResultStoreTransposed,
};
use dioxus::prelude::*;
use query_parse::parse_expr;
use ui::{DataTable, value_to_string};

#[component]
pub fn ExprPage() -> Element {
    let db = use_context::<Db>();

    let mut text_value = use_signal(|| String::new());

    let expr = text_value.with(|value| parse_expr(value));

    let expr_result = expr.as_ref().map(|expr| db.run_expr(expr));

    let query = use_memo(move || {
        println!("parsing query");
        query_parse::parse(&text_value.read())
    });

    let mut query_result = use_store({
        let db = db.clone();
        move || {
            println!("creating store");
            Some(UiQueryResult::new(query()?, &db))
        }
    });

    use_effect(move || {
        if let Some(query) = query() {
            println!("updating store");
            query_result.set(Some(UiQueryResult::new(query, &db)));
        }
    });

    let query_button = |query: &'static str| {
        rsx! {
            button {
            onclick: move |_| text_value.set(query.to_owned()),
            "{query}"
        }
        }
    };

    rsx! {
        textarea {
            class: "query-input",
            placeholder: "Query/Expr",
            spellcheck: false,
            autocomplete: false,
            cols: 80,
            rows: 12,
            oninput: move |e| text_value.set(e.value()),
            value: "{text_value}"
        }
        {query_button("query project group_by project.group")}
        {query_button("query work_time where work_time.project->is_fun")}
        {query_button("query work_time group_by work_time.project->group")}
        {query_button("query project group_by project.group group_extra sum(project.priority)")}
        div {
            "Expr: {expr:?}",
        }
        div {
            "Result: {expr_result:?}",
        }
        div {
            "Query: {query:?}",
        }
        if let Some(result) = query_result.transpose()
        {
            match result.result().transpose() {
                Ok(result) => rsx! {
                    QueryResultView { result }
                },
                Err(err) => rsx! {"{err:?}"}
            }
        }
    }
}

#[component]
pub fn QueryResultView(result: Store<QueryResult>) -> Element {
    match result.transpose() {
        QueryResultStoreTransposed::Records(records) => {
            rsx! {
                DataTable {
                    records: records,
                    table_name: "Foobar",
                }
            }
        }
        QueryResultStoreTransposed::Grouped { groups } => {
            let db = use_context::<Db>();

            rsx! {
                for group in groups.iter() {
                    div {
                        class: "query-result-group",
                        h1 { "{value_to_string(group.group().read().clone(), &db)}" }
                        if let Some(extra) = group.extra().transpose() {
                            h2 {"{value_to_string(extra.read().clone(), &db)}"}
                        }
                        QueryResultView { result: group.result() }
                    }
                }
            }
        }
    }
}

#[derive(Store)]
struct UiQueryResult {
    query: Query,
    result: Result<QueryResult, DbError>,
}

impl UiQueryResult {
    pub fn new(query: Query, db: &Db) -> Self {
        let result = db.run_query(&query);

        Self { query, result }
    }
}
