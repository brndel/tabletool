

use std::fs;

use dioxus::prelude::*;

use ui::Navbar;
use views::{Info, Home, TablePage};

mod views;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(DesktopNavbar)]
    #[route("/")]
    Home {},
    #[route("/table/:name")]
    TablePage { name: String },
    #[route("/info")]
    Info { },
}

const MAIN_CSS: Asset = asset!("/assets/main.css");
const COMPONENT_CSS: Asset = asset!("/assets/dx-components-theme.css");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    // Build cool things ✌️

    use_context_provider(|| db::Db::new("data/main.db").unwrap());

    rsx! {
        // Global app resources
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Link { rel: "stylesheet", href: COMPONENT_CSS }
        document::Title { "Tabletool" }

        Router::<Route> {}
    }
}

/// A desktop-specific Router around the shared `Navbar` component
/// which allows us to use the desktop-specific `Route` enum.
#[component]
fn DesktopNavbar() -> Element {
    rsx! {
        Navbar {
            Link {
                to: Route::Home {},
                "Home"
            }
            Link {
                to: Route::Info {},
                "Info"
            }
        }

        Outlet::<Route> {}
    }
}

