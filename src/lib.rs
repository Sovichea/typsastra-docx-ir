//! Renderer-independent types for source-aware, paginated DOCX layout.
//!
//! This crate models document structure after layout but before painting. It is
//! intended for editing tools, layout regression tests, and AI feedback loops.

use serde::{Deserialize, Serialize};

pub const FORMAT: &str = "typsastra-docx-ir";
pub const VERSION: &str = "1.0";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DocumentLayout {
    pub format: String,
    pub version: String,
    pub generator: GeneratorInfo,
    pub source: SourceInfo,
    pub pages: Vec<PageLayout>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
}

impl DocumentLayout {
    pub fn new(generator: GeneratorInfo, source: SourceInfo, pages: Vec<PageLayout>) -> Self {
        Self {
            format: FORMAT.into(),
            version: VERSION.into(),
            generator,
            source,
            pages,
            diagnostics: Vec::new(),
        }
    }

    pub fn is_supported(&self) -> bool {
        self.format == FORMAT && self.version.starts_with("1.")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratorInfo {
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceInfo {
    pub path: String,
    pub byte_length: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PageLayout {
    pub index: usize,
    pub number: Option<u32>,
    pub size_pt: Size,
    pub content_bbox_pt: Option<Rect>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regions: Vec<Region>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Region {
    Paragraph(ParagraphRegion),
    Image(ImageRegion),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRef {
    pub part: String,
    pub id: String,
    pub identity: IdentityKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityKind {
    ParaId,
    RelationshipId,
    GeneratedPath,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParagraphRegion {
    pub source: SourceRef,
    pub segment: usize,
    pub segment_count: usize,
    pub text: String,
    pub frame_bbox_pt: Rect,
    pub content_bbox_pt: Option<Rect>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<LineLayout>,
    pub overflow: Option<Overflow>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LineLayout {
    /// Half-open UTF-8 byte range into `ParagraphRegion::text`.
    pub range_utf8: [usize; 2],
    pub bbox_pt: Rect,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageRegion {
    pub source: SourceRef,
    pub owner_id: Option<String>,
    pub bbox_pt: Rect,
    pub placement: ImagePlacement,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImagePlacement {
    Inline,
    Floating,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Overflow {
    pub horizontal: bool,
    pub vertical: bool,
    pub clipped: bool,
    pub past_content_area_pt: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn right(self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(self) -> f32 {
        self.y + self.height
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    pub source_id: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_document_uses_v1_identity() {
        let document = DocumentLayout::new(
            GeneratorInfo {
                name: "test".into(),
                version: "0".into(),
            },
            SourceInfo {
                path: "book.docx".into(),
                byte_length: 42,
            },
            Vec::new(),
        );
        assert!(document.is_supported());
        assert_eq!(document.format, FORMAT);
    }

    #[test]
    fn region_serialization_is_tagged() {
        let region = Region::Image(ImageRegion {
            source: SourceRef {
                part: "word/document.xml".into(),
                id: "rId4".into(),
                identity: IdentityKind::RelationshipId,
            },
            owner_id: None,
            bbox_pt: Rect::default(),
            placement: ImagePlacement::Inline,
        });
        let json = serde_json::to_value(region).unwrap();
        assert_eq!(json["kind"], "image");
    }
}
