use db::Ulid;
use dioxus::prelude::*;

#[component]
pub fn IdCard(id: Ulid) -> Element {
    let r = ((id.0 >> 0) & 0xFF) as u8;
    let g = ((id.0 >> 8) & 0xFF) as u8;
    let b = ((id.0 >> 16) & 0xFF) as u8;

    let color = format!("#{r:02X}{g:02X}{b:02X}");
    let s = id_text(id);

    rsx!(span {
        class: "id-card",
        background_color: "{color}",
        color: if use_white_text(r, g, b) {"#ffffff"} else {"#000000"},
        title: "{id}",
        "{s}"
    })
}

pub fn id_text(id: Ulid) -> String {
    let string = id.to_string();

    let last_4 = &string[string.len()-4..string.len()];

    last_4.to_owned()
}

pub fn use_white_text(r: u8, g: u8, b: u8) -> bool {
    fn channel_luminance(c: u8) -> f64 {
        let c = c as f64 / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    let r = channel_luminance(r);
    let g = channel_luminance(g);
    let b = channel_luminance(b);

    let luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;

    let contrast_black = (luminance + 0.05) / 0.05;
    let contrast_white = 1.05 / (luminance + 0.05);

    contrast_white > contrast_black
}