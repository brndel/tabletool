use db::Ulid;
use dioxus::prelude::*;

#[component]
pub fn IdCard(id: Ulid) -> Element {
    let mut seed = id.0;
    fn hash(mut seed: u128) -> u128 {
        seed ^= seed << 7;
        seed ^= seed >> 9;
        seed = seed.wrapping_mul(0x9E3779B97F4A7C15u128); // golden ratio magic
        seed
    }

    seed = hash(seed);

    const CHARS: &[u8] = b"abcdefghjklmnopqrstuvwxyzABCDEFGHJKLMNOPQRSTUVWXYZ0123456789";

    let c1 = CHARS[(seed as usize) % CHARS.len()] as char;
    let c2 = CHARS[((seed >> 8) as usize) % CHARS.len()] as char;
    let c3 = CHARS[((seed >> 16) as usize) % CHARS.len()] as char;
    let c4 = CHARS[((seed >> 24) as usize) % CHARS.len()] as char;

    seed = hash(seed);

    let r = ((seed >> 0) & 0xFF) as u8;
    let g = ((seed >> 8) & 0xFF) as u8;
    let b = ((seed >> 16) & 0xFF) as u8;

    let color = format!("#{r:02X}{g:02X}{b:02X}");
    let s = format!("{}{}{}{}", c1, c2, c3, c4);

    rsx!(span {
        class: "id-card",
        background_color: "{color}",
        "{s}"
    })
}

pub fn id_text(id: Ulid) -> String {
    let mut seed = id.0;
    fn hash(mut seed: u128) -> u128 {
        seed ^= seed << 7;
        seed ^= seed >> 9;
        seed = seed.wrapping_mul(0x9E3779B97F4A7C15u128); // golden ratio magic
        seed
    }

    seed = hash(seed);

    const CHARS: &[u8] = b"abcdefghjklmnopqrstuvwxyzABCDEFGHJKLMNOPQRSTUVWXYZ0123456789";

    let c1 = CHARS[(seed as usize) % CHARS.len()] as char;
    let c2 = CHARS[((seed >> 8) as usize) % CHARS.len()] as char;
    let c3 = CHARS[((seed >> 16) as usize) % CHARS.len()] as char;
    let c4 = CHARS[((seed >> 24) as usize) % CHARS.len()] as char;

    format!("{}{}{}{}", c1, c2, c3, c4)
}