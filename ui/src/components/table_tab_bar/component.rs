use dioxus::{prelude::*, router::{LinkProps, Link}};


#[component]
pub fn TableTabBar(
    children: Element,
    #[props(extends = GlobalAttributes)]
    attributes: Vec<Attribute>,
) -> Element {
    rsx! {
        document::Link { rel: "stylesheet", href: asset!("./style.css") }
        div {
            class: "table-tab-bar",
            ..attributes,
            {children}
        }
    }
}

#[component]
pub fn TableTab(props: LinkProps) -> Element {
    rsx! {
        Link {
            class: Some("table-tab".to_owned()),
            active_class: Some("active".to_owned()),
            ..props
        }
    }
}