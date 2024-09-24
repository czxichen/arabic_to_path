#![allow(unused)]

use unicode_bidi::{LTR_LEVEL, RTL_LEVEL};

mod text;

fn main() {
    let data = "H مِن امْبِرِّ امْصِيامُ في امْسَفَرِ1  H 3 
    123 « F تشکیل";

    let data = " امْصِيامُ في امْسَفَرِ";

    let mut fontdb = fontdb::Database::new();
    fontdb.load_fonts_dir("fonts");
    let text = text::Text {
        x: 350.0,
        y: 100.0,
        text: data.to_string(),
        font_size: 60.0,
        font_step: 0.0,
        text_align: text::Align::C,
        text_limit: None,
        font_weight: Some(700),
        font_family: "Times New Roman".to_string(),
        line_height: Some(30.0),
    };

    let raw = text::text_to_raw(&text, &fontdb).unwrap();
    let document = svg::Document::new().set("viewBox", (0, 0, 700, 200));
    let path = svg::node::element::Path::new().set("d", raw);
    std::fs::write("text.svg", document.add(path).to_string()).unwrap();
}
