use db_core::expr::LineColPos;
// use crate::hotreload::HotReload;
use dioxus::{
    document::document,
    html::{g::mode, mo},
    prelude::*,
};
use dioxus_use_js::use_js;
// use dioxus_sdk::utils::timing::UseDebounce;
// use model::{CargoDiagnostic, CargoLevel};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::code_editor;
// use wasm_bindgen::prelude::*;

// #[cfg(target_arch = "wasm32")]
// use dioxus_sdk::theme::SystemTheme;

/// Get the path prefix for the `/vs` folder inside the Monaco folder.
pub fn monaco_vs_prefix(folder: Asset) -> String {
    let monaco_vs_prefix = format!("{}/vs", folder);
    monaco_vs_prefix
}

/// Get the full path to the `loader.js` script.
pub fn monaco_loader_src(folder: Asset) -> String {
    let prefix = monaco_vs_prefix(folder);
    format!("{prefix}/loader.js")
}

pub struct CodeDiagnostic {
    message: String,
}

pub enum SystemTheme {
    Light,
    Dark,
}

const DXP_CSS: Asset = asset!("/assets/dxp.css");
const MONACO_FOLDER: Asset = asset!("/assets/monaco-editor-0.52.2");

#[component]
pub fn MonacoEditor(
    model: Signal<String>,
    markers: ReadSignal<Vec<Marker>>,
    hover_provider: Callback<HoverRequest, Option<String>>,
    completion_provider: Callback<CompletionRequest, Vec<CompletionItem>>,
) -> Element {
    let mut model_change_guard = use_signal(|| false);

    let on_change_callback = use_callback(move |value| {
        model_change_guard.set(true);
        model.set(value);
    });

    use_effect(move || {
        let model = model.read();
        let model: String = model.clone();

        let guard = *model_change_guard.peek();
        if guard {
            model_change_guard.set(false);
            return;
        }

        let _ = spawn(async move {
            let _ = setCurrentModelValue(&model).await;
        });
    });

    use_effect(move || {
        let markers = markers.read();
        let markers: Vec<_> = markers.clone();

        spawn(async move {
            set_markers(markers.as_ref()).await;
        });
    });

    let on_ready_callback = use_callback(move |()| {
        let markers = markers.read();
        let markers: Vec<_> = markers.clone();

        spawn(async move {
            set_markers(markers.as_ref()).await;
        });
    });

    // let hover_provider = use_callback(move |request: HoverRequest| {
    //     let pos = LineColPos { line: request.position.line_number, col: request.position.column };

    //     let model = &model.peek();
    //     let idx = pos.to_index(&model);

    //     println!("hover request on {}", idx);
    //     if request.position.column <= 4 {
    //         return None;
    //     } else {
    //         return Some(format!("**Hello** from Rust\nRequest for:\n{:?}", request));
    //     }
    // });

    rsx! {
        script {
            src: monaco_loader_src(MONACO_FOLDER),
            onload: move |_| async move {
                let model = model.peek();
                on_monaco_load(
                    MONACO_FOLDER,
                    SystemTheme::Dark,
                    &model,
                    on_ready_callback,
                    on_change_callback,
                    hover_provider,
                    completion_provider,
                ).await;
            }
        }

        div {
            id: code_editor::EDITOR_ELEMENT_ID
        }
    }
}

use_js!("src/code_editor/monaco.ts", "src/code_editor/monaco.js"::*);

/// Initialize Monaco once the loader script loads.
// #[cfg(target_arch = "wasm32")]
pub async fn on_monaco_load(
    folder: Asset,
    system_theme: SystemTheme,
    contents: &str,
    // mut hot_reload: HotReload,
    on_ready_callback: Callback<(), ()>,
    model_callback: Callback<String, ()>,
    hover_provider: Callback<HoverRequest, Option<String>>,
    completion_provider: Callback<CompletionRequest, Vec<CompletionItem>>,
) {
    // let on_ready_callback = Closure::new(move || monaco_ready.set(true));
    let monaco_prefix = monaco_vs_prefix(folder);
    init(
        &monaco_prefix,
        super::EDITOR_ELEMENT_ID,
        system_theme,
        contents,
        on_ready_callback,
        hover_provider,
        completion_provider,
    )
    .await;

    // // hot_reload.set_starting_code(contents);

    let model_change_callback = use_callback(move |new_code: String| async move {
        model_callback(new_code);
        Ok(true)
    });
    registerModelChangeEvent(model_change_callback)
        .await
        .unwrap();

    // on_ready_callback.forget();
    // model_change_callback.forget();
    // log_hello();
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HoverRequest {
    pub position: HoverPosition,
    // pub content: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HoverPosition {
    pub line_number: usize,
    pub column: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CompletionRequest {
    pub position: HoverPosition,
    // pub content: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionKind,
    pub insert_text: String,
}

impl CompletionItem {
    pub fn new(text: String, kind: CompletionKind) -> Self {
        Self {
            label: text.clone(),
            kind,
            insert_text: text,
        }
    }
}

#[derive(Serialize_repr, Deserialize_repr, Clone, Debug)]
#[repr(u8)]
pub enum CompletionKind {
    Method = 0,
    Function,
    Constructor,
    Field,
    Variable,
    Class,
    Struct,
    Interface,
    Module,
    Property,
    Event,
    Operator,
    Unit,
    Value,
    Constant,
    Enum,
    EnumMember,
    Keyword,
    Text,
    Color,
    File,
    Reference,
    Customcolor,
    Folder,
    TypeParameter,
    User,
    Issue,
    Snippet,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Marker {
    pub message: String,
    pub severity: MarkerSeverity,

    #[serde(rename = "startLineNumber")]
    pub start_line_number: usize,

    #[serde(rename = "endLineNumber")]
    pub end_line_number: usize,

    #[serde(rename = "startColumn")]
    pub start_column: usize,

    #[serde(rename = "endColumn")]
    pub end_column: usize,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum MarkerSeverity {
    Hint,
    Info,
    Warning,
    Error,
}

async fn set_markers(markers: &[Marker]) {
    // let data = serde_wasm_bindgen::to_value(markers).unwrap();
    // set_model_marker(data);
    setModelMarkers(markers).await;
}

// Bindings
// #[wasm_bindgen(module = "/src/code_editor/monaco.js")]
// extern "C" {
//     #[wasm_bindgen(js_name = initMonaco)]
//     fn init_monaco(
//         vs_path_prefix: &str,
//         element_id: &str,
//         initial_theme: &str,
//         initial_snippet: &str,
//         on_ready_callback: &Closure<dyn FnMut()>,
//     );

//     #[wasm_bindgen(js_name = getCurrentModelValue)]
//     pub fn get_current_model_value() -> String;

//     #[wasm_bindgen(js_name = setCurrentModelvalue)]
//     pub fn set_current_model_value(value: &str);

//     #[wasm_bindgen(js_name = isReady)]
//     pub fn is_ready() -> bool;

//     #[wasm_bindgen(js_name = setTheme)]
//     fn set_monaco_theme(theme: &str);

//     #[wasm_bindgen(js_name = setModelMarkers)]
//     fn set_model_marker(markers: JsValue);

//     #[wasm_bindgen(js_name = registerPasteAsRSX)]
//     fn register_paste_as_rsx(convertHtmlToRSX: &Closure<dyn Fn(String) -> Option<String>>);

//     #[wasm_bindgen(js_name = registerModelChangeEvent)]
//     fn register_model_change_event(callback: &Closure<dyn FnMut(String)>);

//     #[wasm_bindgen(js_name = logHello)]
//     fn log_hello();
// }

pub async fn init(
    vs_path_prefix: &str,
    element_id: &str,
    initial_theme: SystemTheme,
    initial_snippet: &str,
    on_ready_callback: Callback<(), ()>,
    hover_provider: Callback<HoverRequest, Option<String>>,
    completion_provider: Callback<CompletionRequest, Vec<CompletionItem>>,
) {
    let theme = system_theme_to_string(initial_theme);
    let ready_callback = use_callback(move |v: bool| async move {
        println!("MONACO IS READY");
        on_ready_callback(());

        Ok(())
    });

    let hover_callback = use_callback(move |input: dioxus_use_js::SerdeJsonValue| async move {
        let request = serde_json::from_value(input).map_err(|err| format!("{err:?}"))?;
        let result = hover_provider(request);

        Ok(result)
    });

    let completion_provider =
        use_callback(move |input: dioxus_use_js::SerdeJsonValue| async move {
            let request = serde_json::from_value(input).map_err(|err| format!("{err:?}"))?;
            let result = completion_provider(request);

            Ok(serde_json::to_value(&result).unwrap())
        });

    initMonaco(
        vs_path_prefix,
        element_id,
        &theme,
        initial_snippet,
        ready_callback,
        hover_callback,
        completion_provider,
    )
    .await
    .unwrap();
}

#[cfg(target_arch = "wasm32")]
pub fn set_theme(theme: SystemTheme) {
    let theme = system_theme_to_string(theme);
    set_monaco_theme(&theme);
}

fn system_theme_to_string(theme: SystemTheme) -> String {
    match theme {
        SystemTheme::Light => "dx-vs",
        SystemTheme::Dark => "dx-vs-dark",
    }
    .to_string()
}
