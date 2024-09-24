#![allow(unused)]

use fontdb::{Database, Family, Weight};
use rustybuzz::{
    ttf_parser::{GlyphId, OutlineBuilder},
    UnicodeBuffer,
};
use serde::{Deserialize, Serialize};
use std::fmt::{Error, Write};
use tiny_skia_path::{Path, PathBuilder, PathSegment, Transform};
use unicode_bidi::{Level, LTR_LEVEL, RTL_LEVEL};

#[macro_export]
macro_rules! map {
    ($($key:expr => $value:expr),* $(,)?) => ({
        let mut m = std::collections::HashMap::new();
        $(m.insert($key, $value);)*
        m
    });
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Align {
    R,  // 右对齐
    L,  // 左对齐
    M,  // 中间对齐
    C,  // 中心对齐
    CR, // 右中心对齐
    CL, // 左中心对齐
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Text {
    pub x: f32,              // 开始X轴位置
    pub y: f32,              // 开始Y轴位置
    pub text: String,        // 文本
    pub font_size: f32,      // 字号
    pub font_step: f32,      // 字间距
    pub font_family: String, // 文本字体
    pub text_align: Align,   // 文本对齐方式
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_limit: Option<(f32, f32)>, // 文本宽高限制
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_weight: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_height: Option<f32>,
}

impl Text {
    pub fn new(text: String, font_size: f32, font_family: String) -> Self {
        return Text {
            x: 0.0,
            y: 0.0,
            text,
            font_size,
            font_step: 0.0,
            text_align: Align::L,
            text_limit: None,
            font_weight: None,
            font_family: font_family,
            line_height: Some(font_size / 2.0),
        };
    }
    fn to_path_with_font(&self, font: &[u8]) -> Option<Path> {
        let face = rustybuzz::Face::from_slice(font, 0)?;
        let scale_x = self.font_size / face.units_per_em() as f32;
        let scale_y = -scale_x;
        let mut path_builder = PathBuilder::new();

        let space_advance_width = face
            .glyph_hor_advance(face.glyph_index(' ').unwrap_or_default())
            .unwrap_or_default() as f32
            * scale_x;

        let mut current_x = 0.0;
        let mut current_y = 0.0;
        let mut bidi_info = unicode_bidi::BidiInfo::new(&self.text, None);
        if bidi_info.levels.iter().any(|v| v.is_ltr()) {
            bidi_info = unicode_bidi::BidiInfo::new(&self.text, Some(LTR_LEVEL));
        }
        let mut height = 0.0f32;
        for para in &bidi_info.paragraphs {
            let source = &self.text[para.range.clone()];
            let ts = source.trim_end();
            let mut line = para.range.clone();
            line.end -= source.len() - ts.len();
            let (info, rgs) = bidi_info.visual_runs(para, line);
            for rg in rgs.iter() {
                let mut buffer = UnicodeBuffer::new();
                buffer.push_str(&self.text[rg.clone()]);
                buffer.guess_segment_properties();
                buffer.set_direction(if para.level.is_rtl() {
                    rustybuzz::Direction::RightToLeft
                } else {
                    rustybuzz::Direction::LeftToRight
                });
                let output = rustybuzz::shape(&face, &[], buffer);
                for (info, pos) in output
                    .glyph_infos()
                    .iter()
                    .zip(output.glyph_positions().iter())
                {
                    let mut builder = RawPathBuilder::new();
                    if let Some(rect) =
                        face.outline_glyph(GlyphId(info.glyph_id as u16), &mut builder)
                    {
                        let mut path = builder.current.finish()?;
                        path = path.transform(
                            Transform::identity()
                                .pre_scale(scale_x, scale_y)
                                .post_translate(
                                    current_x + pos.x_offset as f32 * scale_x,
                                    current_y + pos.y_offset as f32 * scale_y,
                                ),
                        )?;

                        current_x += pos.x_advance as f32 * scale_x;
                        current_y += pos.y_advance as f32 * scale_y;
                        path_builder.push_path(&path);

                        height =
                            height.max((rect.height() as f32 + pos.y_advance as f32) * scale_x);
                    } else {
                        current_x += space_advance_width;
                    }
                    current_x += self.font_step;
                }
            }
            current_x = 0.0;
            current_y += (height + self.line_height.unwrap_or_default());
            height = 0.0;
        }

        let mut path = path_builder.finish()?;
        if let Some(rect) = path.compute_tight_bounds() {
            path =
                path.transform(Transform::identity().pre_translate(-rect.left(), -rect.bottom()))?;
        }

        if let Some(limit) = self.text_limit {
            let bound = path.bounds();
            let (w, h) = (
                (bound.right() - bound.left()).abs(),
                (bound.top() - bound.bottom()).abs(),
            );

            let ws = if limit.0 < w && limit.0 > 0.0 {
                limit.0 / w
            } else {
                1.0
            };
            let hs = if limit.1 < h && limit.1 > 0.0 {
                limit.1 / h
            } else {
                1.0
            };
            let scale = ws.min(hs);
            if scale != 1.0 {
                path = path.transform(Transform::identity().pre_scale(scale, scale))?;
            }
        }

        let bound = path.bounds();
        let (w, h) = (
            (bound.right() - bound.left()).abs(),
            (bound.top() - bound.bottom()).abs(),
        );

        let mut ts = Transform::identity();
        match self.text_align {
            Align::L => ts = ts.post_translate(self.x, self.y),
            Align::R => {
                ts = ts.post_translate(self.x - w, self.y);
            }
            Align::M => {
                ts = ts.post_translate(self.x - w / 2.0, self.y);
            }
            Align::C => {
                ts = ts.post_translate(self.x - w / 2.0, self.y + h / 2.0);
            }
            Align::CL => {
                ts = ts.post_translate(self.x, self.y + h / 2.0);
            }
            Align::CR => {
                ts = ts.post_translate(self.x - w, self.y + h / 2.0);
            }
        };
        return path.transform(ts);
    }

    pub fn to_path(&self, fontdb: &Database) -> Option<Path> {
        let query = fontdb::Query {
            families: &[Family::Name(&self.font_family)],
            weight: Weight(self.font_weight.unwrap_or(400)),
            ..Default::default()
        };
        fontdb.with_face_data(fontdb.query(&query)?, |data, _| {
            self.to_path_with_font(data)
        })?
    }
}

pub fn path_to_raw(path: &Path) -> Result<String, Error> {
    let mut raw = String::new();
    for segment in path.segments() {
        match segment {
            PathSegment::MoveTo(p) => raw.write_fmt(format_args!("M {} {} ", p.x, p.y))?,
            PathSegment::LineTo(p) => raw.write_fmt(format_args!("L {} {} ", p.x, p.y))?,
            PathSegment::QuadTo(p0, p1) => {
                raw.write_fmt(format_args!("Q {} {} {} {} ", p0.x, p0.y, p1.x, p1.y))?
            }
            PathSegment::CubicTo(p0, p1, p2) => raw.write_fmt(format_args!(
                "C {} {} {} {} {} {} ",
                p0.x, p0.y, p1.x, p1.y, p2.x, p2.y
            ))?,
            PathSegment::Close => raw.write_fmt(format_args!("Z "))?,
        }
    }
    raw.pop();
    return Ok(raw);
}

pub fn text_to_raw(text: &Text, fontdb: &Database) -> Result<String, String> {
    let path = if let Some(path) = text.to_path(fontdb) {
        path
    } else {
        return Err("text to path error check font".to_string());
    };
    return path_to_raw(&path).map_err(|err| err.to_string());
}

pub struct RawPathBuilder {
    pub current: PathBuilder,
}

impl RawPathBuilder {
    pub fn new() -> Self {
        RawPathBuilder {
            current: PathBuilder::new(),
        }
    }
}

impl OutlineBuilder for RawPathBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.current.move_to(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.current.line_to(x, y);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.current.quad_to(x1, y1, x, y);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.current.cubic_to(x1, y1, x2, y2, x, y);
    }

    fn close(&mut self) {
        self.current.close();
    }
}

#[test]
fn test_path() {
    let data = "مِن امْبِرِّ امْصِيامُ في امْسَفَرِ1  3 
123 « F تشکیل";

    let mut fontdb = fontdb::Database::new();
    fontdb.load_fonts_dir("/Users/djl9460/data/code/rust/AiPlugin/ai_server/doc/fonts");
    let text = Text {
        x: 0.0,
        y: 0.0,
        text: data.to_string(),
        font_size: 60.0,
        font_step: 0.0,
        text_align: Align::C,
        text_limit: None,
        font_weight: Some(700),
        font_family: "Times New Roman".to_string(),
        line_height: Some(30.0),
    };

    let raw = text_to_raw(&text, &fontdb).unwrap();
    let document = svg::Document::new().set("viewBox", (0, 0, 200, 500));
    let path = svg::node::element::Path::new().set("d", raw);
    std::fs::write("text.svg", document.add(path).to_string()).unwrap();
}
