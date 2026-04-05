use db::{CompiledQuery, Db, DbError};
use db_core::{
    asm_code::CompilerDiagnostics,
    expr::{LineColPos, LineColSpan},
    query::{
        Query, QueryResult, QueryResultGroupStoreExt, QueryResultStoreExt,
        QueryResultStoreTransposed,
    },
};
use dioxus::{document::document, prelude::*};
use query_parse::parse_expr;
use ui::{DataTable, value_to_string};

use crate::code_editor::{
    self,
    monaco::{
        CompletionItem, CompletionKind, CompletionRequest, HoverRequest, Marker, MonacoEditor,
        SystemTheme, monaco_loader_src, on_monaco_load,
    },
};

#[component]
pub fn ExprPage() -> Element {
    let db = use_context::<Db>();

    let mut text_value = use_signal(|| String::new());
    let mut markers = use_signal(|| vec![]);
    let mut query = use_signal(|| Option::<Query>::None);

    // let expr = text_value.with(|value| parse_expr(value));

    // let expr_result = expr.as_ref().map(|expr| db.run_expr(expr));

    use_effect(move || {
        let query_code = text_value.read();
        let query_code: &str = &query_code;

        let (query_result, errors) = query_parse::parse(&query_code);

        query.set(query_result);

        let error_markers = errors.into_iter().map(|err| {
            let span = err.span();
            let span = LineColSpan::from_simple_span(span, query_code);
            Marker {
                message: err.content(),
                severity: code_editor::monaco::MarkerSeverity::Error,
                start_line_number: span.line_start,
                end_line_number: span.line_end,
                start_column: span.col_start,
                end_column: span.col_end,
            }
        });

        markers.set(error_markers.collect());
    });

    let mut query_result = use_store({
        let db = db.clone();
        move || Some(UiQueryResult::new(query()?, &db))
    });

    let mut query_db_result = use_store({
        let db = db.clone();
        move || {
            let query = query_result.read();

            match query.as_ref() {
                Some(UiQueryResult {
                    result: Ok(query), ..
                }) => Some(db.run_query(query)),
                _ => None,
            }
        }
    });

    use_effect(move || {
        if let Some(query) = query.read().clone() {
            let result = UiQueryResult::new(query, &db);

            let diagnostics = match &result.result {
                Ok(result) => &result.diagnostics,
                Err(diagnostics) => diagnostics,
            };

            let text_value = text_value.peek();

            let diagnostics_markers = diagnostics
                .markers()
                .iter()
                .map(|marker| {
                    let span = LineColSpan::from_simple_span(&marker.span, &text_value);

                    Marker {
                        message: marker.value.to_string(),
                        severity: match marker.value.kind() {
                            db_core::asm_code::MarkerKind::Error => {
                                code_editor::monaco::MarkerSeverity::Error
                            }
                            db_core::asm_code::MarkerKind::Warning => {
                                code_editor::monaco::MarkerSeverity::Warning
                            }
                        },
                        start_line_number: span.line_start,
                        end_line_number: span.line_end,
                        start_column: span.col_start,
                        end_column: span.col_end,
                    }
                })
                .collect();

            markers.set(diagnostics_markers);

            let db_result = match &result {
                UiQueryResult {
                    result: Ok(query), ..
                } => Some(db.run_query(query)),
                _ => None,
            };

            query_result.set(Some(result));
            query_db_result.set(db_result);
            println!("query_result updated");
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

    let hover_provider = use_callback(move |hover_request: HoverRequest| {
        let content = text_value.peek();

        let index = LineColPos {
            line: hover_request.position.line_number,
            col: hover_request.position.column,
        }
        .to_index(&content);

        let result = query_result.peek();
        let result_value = result.as_ref()?;

        let diagnostics = match &result_value.result {
            Ok(result) => &result.diagnostics,
            Err(diagnostics) => diagnostics,
        };

        let highlight = diagnostics.highlights().get_at(index)?;

        Some(highlight.value.message.clone())
    });

    let completion_provider = use_callback(move |req: CompletionRequest| {
        println!("completion for {req:?}");

        let content = text_value.peek();

        let index = LineColPos {
            line: req.position.line_number,
            col: req.position.column,
        }
        .to_index(&content);

        let result = query_result.peek();
        let Some(result_value) = result.as_ref() else {
            return Vec::new();
        };

        let diagnostics = match &result_value.result {
            Ok(result) => &result.diagnostics,
            Err(diagnostics) => diagnostics,
        };

        let highlight = diagnostics.completions().get_before(index);

        match highlight {
            Some(value) => value
                .value
                .options
                .iter()
                .map(|text| CompletionItem::new(text.clone(), CompletionKind::Field))
                .collect(),
            None => Vec::new(),
        }
    });

    rsx! {

        MonacoEditor { model: text_value, markers, hover_provider, completion_provider }
        
        {query_button("query project group_by project.group")}
        {query_button("query work_time where work_time.project.is_fun")}
        {query_button("query work_time group_by work_time.project.group")}
        {query_button("query project group_by project.group group_extra sum(project.priority)")}
        // div {
        //     "Expr: {expr:?}",
        // }
        // div {
        //     "Result: {expr_result:?}",
        // }
        // div {
        //     "Query: {query:?}",
        // }
        match query_db_result.transpose() {
            Some(result) => match result.transpose() {
                Ok(result) => rsx! {
                    QueryResultView { result }
                },
                Err(err) => rsx! {"Error while running query: {err:?}"}
            }
            None => rsx!("query did not compile")
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
    result: Result<CompiledQuery, CompilerDiagnostics>,
}

impl UiQueryResult {
    pub fn new(query: Query, db: &Db) -> Self {
        let result = db.compile_query(&query);

        Self { query, result }
    }
}
