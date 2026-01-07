use db::Db;
use db_core::expr::{EvalCtx, TyCtx};
use dioxus::prelude::*;
use query_parse::parse;

#[component]
pub fn ExprPage() -> Element {
    let db = use_context::<Db>();

    let mut expr_value = use_signal(|| String::new());

    let query = expr_value.with(|value| parse(value));

    let result = query.as_ref().map(|query| db.run_query(query));

    // let ty = expr.as_ref().and_then(|expr| expr.ty(&TyCtx::default()));
    // let result = expr.as_ref().and_then(|expr| expr.eval(&EvalCtx::default()));

    rsx! {
        "Expr",
        textarea {
            placeholder: "Expr",
            oninput: move |e| expr_value.set(e.value()),
        },
        div {
            "Query: {query:?}",
        }
        div {
            "Result: {result:?}",
        }
    }
}
