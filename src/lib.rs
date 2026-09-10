//! Renderer-independent types for source-aware, paginated DOCX layout.
//!
//! This crate models document structure after layout but before painting. It is
//! intended for editing tools, layout regression tests, and AI feedback loops.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod compatibility;
pub mod limits;
pub mod provenance;
pub mod serialization;
pub mod source_identity;
pub mod validation;

pub use compatibility::{CompatibilityError, FormatVersion};
pub use limits::{IrStats, LimitError, PackageBudget, ProcessingLimits, ReadError, read_document};
pub use provenance::{
    EmptyParagraphSegments, PaginatedParagraph, ParagraphSegmentDraft, Sourced,
    finish_paginated_paragraph,
};
pub use serialization::{CanonicalJsonError, to_canonical_json, to_canonical_json_with_limits};
pub use source_identity::{ParagraphIdentity, SourceIdentityError, extract_paragraph_identities};
pub use validation::{ValidationError, ValidationErrors};

pub const FORMAT: &str = "typsastra-docx-ir";
pub const VERSION: &str = "1.0";
pub const SUPPORTED_MAJOR_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentLayout {
    pub format: String,
    pub version: String,
    pub generator: GeneratorInfo,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_environment: Option<LayoutEnvironment>,
    pub source: SourceInfo,
    /// Producer-declared measurement coverage, not independent verification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<MeasurementCoverage>,
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
            layout_environment: None,
            coverage: None,
            source,
            pages,
            diagnostics: Vec::new(),
        }
    }

    pub fn compatibility(&self) -> Result<FormatVersion, CompatibilityError> {
        compatibility::check(&self.format, &self.version)
    }

    pub fn is_supported(&self) -> bool {
        self.compatibility().is_ok()
    }

    pub fn validate(&self) -> Result<(), ValidationErrors> {
        validation::validate(self)
    }
}

/// Global coverage of measurements across all applicable document content.
/// Omitted dimensions are unknown; absence of records never establishes coverage.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct MeasurementCoverage {
    pub pagination: CoverageStatus,
    pub text_geometry: CoverageStatus,
    pub resolved_typography: CoverageStatus,
    pub table_geometry: CoverageStatus,
    pub drawing_appearance: CoverageStatus,
    pub composition_hierarchy: CoverageStatus,
    pub overflow: CoverageStatus,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CoverageStatus {
    /// The producer did not declare whether measurements were captured.
    #[default]
    Unknown,
    /// Measurements were not captured; this does not mean content is absent.
    Absent,
    /// Only some applicable content or properties were measured.
    Partial,
    /// Measurements cover all applicable content in the document.
    Complete,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GeneratorInfo {
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LayoutEnvironment {
    pub engine: LayoutEngineInfo,
    pub platform: PlatformInfo,
    /// Resolved font faces that were actually used during layout.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fonts: Vec<FontInfo>,
    /// Requested-to-resolved family substitutions observed during layout.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub font_substitutions: Vec<FontSubstitution>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LayoutEngineInfo {
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PlatformInfo {
    pub os: String,
    pub architecture: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FontInfo {
    pub family: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub postscript_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Lowercase hexadecimal SHA-256 of the backing font file, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_sha256: Option<String>,
    /// Zero-based face index within a font collection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face_index: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FontSubstitution {
    pub requested_family: String,
    pub resolved_family: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_postscript_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SourceInfo {
    pub path: String,
    pub byte_length: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PageLayout {
    /// Zero-based physical page index.
    pub index: usize,
    /// Logical page number, if the document defines one.
    pub number: Option<u32>,
    pub size_pt: Size,
    pub content_bbox_pt: Option<Rect>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regions: Vec<Region>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Region {
    Paragraph(ParagraphRegion),
    Image(ImageRegion),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct SourceRef {
    /// Canonical package-relative OPC part containing the editable source node.
    /// The part is an inseparable component of the complete source identity.
    pub part: String,
    /// Native ID or deterministic physical XML path within `part`.
    pub id: String,
    pub identity: IdentityKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IdentityKind {
    ParaId,
    RelationshipId,
    GeneratedPath,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ParagraphRegion {
    pub source: SourceRef,
    /// Zero-based page-segment index for a paragraph split across pages.
    pub segment: usize,
    pub segment_count: usize,
    /// Logical paragraph text stored once for all fitted lines.
    pub text: String,
    /// Allocated flow box, including paragraph spacing.
    pub frame_bbox_pt: Rect,
    /// Union of visible line and inline-object boxes.
    pub content_bbox_pt: Option<Rect>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<LineLayout>,
    pub overflow: Option<Overflow>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LineLayout {
    /// Half-open UTF-8 byte range into `ParagraphRegion::text`.
    pub range_utf8: [usize; 2],
    pub bbox_pt: Rect,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ImageRegion {
    pub source: SourceRef,
    pub owner_id: Option<String>,
    pub bbox_pt: Rect,
    pub placement: ImagePlacement,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImagePlacement {
    Inline,
    Floating,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Overflow {
    pub horizontal: bool,
    pub vertical: bool,
    pub clipped: bool,
    pub past_content_area_pt: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    pub source_id: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

pub fn json_schema() -> schemars::Schema {
    schemars::schema_for!(DocumentLayout)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_document() -> DocumentLayout {
        DocumentLayout::new(
            GeneratorInfo {
                name: "test".into(),
                version: "0".into(),
            },
            SourceInfo {
                path: "book.docx".into(),
                byte_length: 42,
            },
            Vec::new(),
        )
    }

    #[test]
    fn new_document_uses_v1_identity() {
        let document = empty_document();
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

    #[test]
    fn schema_names_the_document_type() {
        let schema = serde_json::to_value(json_schema()).unwrap();
        assert_eq!(schema["title"], "DocumentLayout");
    }
}
