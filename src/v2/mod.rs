//! Format 2.0 layout contract for resolved appearance and composition.

use std::{
    collections::{HashMap, HashSet},
    fmt,
    io::Write,
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{CompatibilityError, FORMAT, FormatVersion, IrStats, LimitError, ProcessingLimits};
pub use crate::{
    CoverageStatus, Diagnostic, FontInfo, FontSubstitution, GeneratorInfo, IdentityKind,
    LayoutEngineInfo, LayoutEnvironment, PlatformInfo, Severity, SourceInfo, SourceRef,
};

pub const VERSION: &str = "2.0";
pub const SUPPORTED_MAJOR_VERSION: u16 = 2;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentLayout {
    pub format: String,
    pub version: String,
    pub generator: GeneratorInfo,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_environment: Option<LayoutEnvironment>,
    pub source: SourceInfo,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<MeasurementCoverage>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<SectionLayout>,
    pub pages: Vec<PageLayout>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
}

impl DocumentLayout {
    pub fn new(
        generator: GeneratorInfo,
        source: SourceInfo,
        sections: Vec<SectionLayout>,
        pages: Vec<PageLayout>,
    ) -> Self {
        Self {
            format: FORMAT.into(),
            version: VERSION.into(),
            generator,
            layout_environment: None,
            source,
            coverage: None,
            sections,
            pages,
            diagnostics: Vec::new(),
        }
    }

    pub fn compatibility(&self) -> Result<FormatVersion, CompatibilityError> {
        crate::compatibility::check_major(&self.format, &self.version, SUPPORTED_MAJOR_VERSION)
    }

    pub fn is_supported(&self) -> bool {
        self.compatibility().is_ok()
    }
    pub fn validate(&self) -> Result<(), ValidationErrors> {
        validate(self)
    }

    pub fn stats(&self) -> IrStats {
        let mut stats = IrStats {
            pages: self.pages.len() as u64,
            diagnostics: self.diagnostics.len() as u64,
            ..Default::default()
        };
        let value = serde_json::to_value(self).unwrap_or(Value::Null);
        count_value(&value, &mut stats);
        stats.regions = self.pages.iter().map(|p| p.regions.len() as u64).sum();
        stats.lines = self
            .pages
            .iter()
            .flat_map(|p| &p.regions)
            .filter_map(|r| match r {
                Region::Paragraph(p) => Some(p.lines.len() as u64),
                _ => None,
            })
            .sum();
        stats.text_bytes = self
            .pages
            .iter()
            .flat_map(|p| &p.regions)
            .filter_map(|r| match r {
                Region::Paragraph(p) => Some(p.text.len() as u64),
                _ => None,
            })
            .sum();
        stats.elements = stats
            .elements
            .saturating_add(stats.pages)
            .saturating_add(stats.regions)
            .saturating_add(stats.lines)
            .saturating_add(stats.diagnostics);
        stats
    }

    pub fn check_limits(&self, limits: &ProcessingLimits) -> Result<(), LimitError> {
        check_limit(
            "source document bytes",
            self.source.byte_length,
            limits.max_document_bytes,
        )?;
        let s = self.stats();
        for (name, actual, limit) in [
            ("pages", s.pages, limits.max_pages),
            ("regions", s.regions, limits.max_regions),
            ("lines", s.lines, limits.max_lines),
            ("diagnostics", s.diagnostics, limits.max_diagnostics),
            ("paragraph text bytes", s.text_bytes, limits.max_text_bytes),
            ("IR string bytes", s.string_bytes, limits.max_string_bytes),
            ("IR elements", s.elements, limits.max_elements),
        ] {
            check_limit(name, actual, limit)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct MeasurementCoverage {
    pub pagination: CoverageStatus,
    pub section_geometry: CoverageStatus,
    pub story_content: CoverageStatus,
    pub text_geometry: CoverageStatus,
    pub resolved_typography: CoverageStatus,
    pub resolved_paragraph_style: CoverageStatus,
    pub table_geometry: CoverageStatus,
    pub table_appearance: CoverageStatus,
    pub drawing_geometry: CoverageStatus,
    pub drawing_appearance: CoverageStatus,
    pub composition_hierarchy: CoverageStatus,
    pub reading_order: CoverageStatus,
    pub role_provenance: CoverageStatus,
    pub overflow: CoverageStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct RegionId(pub String);
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct SectionId(pub String);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SectionLayout {
    pub id: SectionId,
    pub index: usize,
    pub source: SourceRef,
    /// Half-open physical page range.
    pub page_range: [usize; 2],
    pub break_kind: SectionBreakKind,
    pub title_page: bool,
    pub different_odd_even: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SectionBreakKind {
    NextPage,
    Continuous,
    EvenPage,
    OddPage,
    NextColumn,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PageLayout {
    pub index: usize,
    pub number: Option<u32>,
    pub size_pt: Size,
    pub orientation: PageOrientation,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub section_geometries: Vec<SectionPageGeometry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regions: Vec<Region>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PageOrientation {
    Portrait,
    Landscape,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Point {
    pub x: f32,
    pub y: f32,
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
pub struct EdgeInsets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SectionPageGeometry {
    pub section_id: SectionId,
    pub margins_pt: EdgeInsets,
    pub gutter_pt: f32,
    pub usable_content_bbox_pt: Rect,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<ColumnLayout>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ColumnLayout {
    pub index: usize,
    pub bbox_pt: Rect,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Region {
    Story(StoryRegion),
    Paragraph(ParagraphRegion),
    Table(TableRegion),
    TableRow(TableRowRegion),
    TableCell(TableCellRegion),
    Image(ImageRegion),
    Shape(ShapeRegion),
}

impl Region {
    pub fn common(&self) -> &RegionCommon {
        match self {
            Self::Story(v) => &v.common,
            Self::Paragraph(v) => &v.common,
            Self::Table(v) => &v.common,
            Self::TableRow(v) => &v.common,
            Self::TableCell(v) => &v.common,
            Self::Image(v) => &v.common,
            Self::Shape(v) => &v.common,
        }
    }
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Story(_) => "story",
            Self::Paragraph(_) => "paragraph",
            Self::Table(_) => "table",
            Self::TableRow(_) => "table_row",
            Self::TableCell(_) => "table_cell",
            Self::Image(_) => "image",
            Self::Shape(_) => "shape",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RegionCommon {
    pub id: RegionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<RegionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<RegionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reading_order: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<RoleAssignment>,
    pub geometry: RegionGeometry,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RoleAssignment {
    pub role: SemanticRole,
    pub provenance: RoleProvenance,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SemanticRole {
    Body,
    Heading { level: u8 },
    ListItem,
    ListLabel,
    Caption,
    Header,
    Footer,
    Footnote,
    Endnote,
    Comment,
    Table,
    TableHeader,
    TableCell,
    Figure,
    Decorative,
    TextBox,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RoleProvenance {
    SourceDeclared {
        declaration: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<SourceRef>,
    },
    Inferred {
        rule: String,
    },
    Defaulted {
        rule: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RegionGeometry {
    /// Axis-aligned envelope in page coordinates.
    pub bbox_pt: Rect,
    pub local_bbox_pt: Rect,
    pub local_to_parent: AffineTransform,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip: Option<ClipGeometry>,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AffineTransform {
    pub m11: f32,
    pub m12: f32,
    pub m21: f32,
    pub m22: f32,
    pub dx: f32,
    pub dy: f32,
}
impl AffineTransform {
    pub const fn identity() -> Self {
        Self {
            m11: 1.0,
            m12: 0.0,
            m21: 0.0,
            m22: 1.0,
            dx: 0.0,
            dy: 0.0,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClipGeometry {
    Rect { rect_pt: Rect },
    Path { path: PathGeometry },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PathGeometry {
    pub fill_rule: FillRule,
    pub commands: Vec<PathCommand>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FillRule {
    NonZero,
    EvenOdd,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PathCommand {
    MoveTo {
        point: Point,
    },
    LineTo {
        point: Point,
    },
    CubicTo {
        control1: Point,
        control2: Point,
        point: Point,
    },
    Close,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StoryRegion {
    #[serde(flatten)]
    pub common: RegionCommon,
    pub source: SourceRef,
    pub story_kind: StoryKind,
    pub occurrence: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub linked_from: Vec<RegionId>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StoryKind {
    Body,
    Header,
    Footer,
    Footnote,
    Endnote,
    Comment,
    TextBox,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PageFragment {
    pub index: usize,
    pub count: usize,
    pub continued_from_previous: bool,
    pub continues_on_next: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ParagraphRegion {
    #[serde(flatten)]
    pub common: RegionCommon,
    pub source: SourceRef,
    pub fragment: PageFragment,
    pub text: String,
    pub frame_bbox_pt: Rect,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_bbox_pt: Option<Rect>,
    pub style: ResolvedParagraphStyle,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<LineLayout>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runs: Vec<ResolvedRun>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<InlineContent>,
    pub overflow: Option<Overflow>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LineLayout {
    pub range_utf8: [usize; 2],
    pub bbox_pt: Rect,
    pub baseline_y_pt: f32,
    pub ascent_pt: f32,
    pub descent_pt: f32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ResolvedRun {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRef>,
    pub range_utf8: [usize; 2],
    pub typography: ResolvedTypography,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ResolvedTypography {
    pub font: ResolvedFont,
    pub size_pt: f32,
    pub color: RgbaColor,
    pub bold: bool,
    pub italic: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underline: Option<TextDecoration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strike: Option<TextDecoration>,
    pub letter_spacing_pt: f32,
    pub horizontal_scale: f32,
    pub baseline_shift_pt: f32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ResolvedFont {
    pub family: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub postscript_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face_index: Option<u32>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TextDecoration {
    pub style: DecorationStyle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<RgbaColor>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DecorationStyle {
    Single,
    Double,
    Dotted,
    Dashed,
    Wavy,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ResolvedParagraphStyle {
    pub alignment: ParagraphAlignment,
    pub direction: TextDirection,
    pub spacing_before_pt: f32,
    pub spacing_after_pt: f32,
    pub line_spacing: ResolvedLineSpacing,
    pub indent_start_pt: f32,
    pub indent_end_pt: f32,
    pub first_line_indent_pt: f32,
    pub borders: BoxBorders,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shading: Option<RgbaColor>,
    pub keep_next: bool,
    pub keep_lines: bool,
    pub widow_control: bool,
    pub page_break_before: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ParagraphAlignment {
    Start,
    End,
    Left,
    Right,
    Center,
    Justified,
    Distributed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TextDirection {
    LeftToRight,
    RightToLeft,
    Vertical,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResolvedLineSpacing {
    Auto { multiplier: f32 },
    Exact { value_pt: f32 },
    AtLeast { value_pt: f32 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InlineContent {
    pub range_utf8: [usize; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRef>,
    #[serde(flatten)]
    pub kind: InlineKind,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InlineKind {
    Text,
    ListLabel,
    Tab {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        alignment: Option<TabAlignment>,
    },
    Break {
        break_kind: BreakKind,
    },
    Field {
        instruction: String,
        result_range_utf8: [usize; 2],
    },
    NoteReference {
        target: RegionId,
    },
    InlineObject {
        target: RegionId,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TabAlignment {
    Left,
    Center,
    Right,
    Decimal,
    Bar,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BreakKind {
    Line,
    Page,
    Column,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TableRegion {
    #[serde(flatten)]
    pub common: RegionCommon,
    pub source: SourceRef,
    pub fragment: PageFragment,
    pub content_bbox_pt: Rect,
    pub alignment: TableAlignment,
    pub layout: TableLayoutMode,
    pub cell_spacing_pt: f32,
    pub borders: BoxBorders,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floating: Option<FloatingPlacement>,
    pub overflow: Option<Overflow>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TableRowRegion {
    #[serde(flatten)]
    pub common: RegionCommon,
    pub source: SourceRef,
    pub fragment: PageFragment,
    pub row_index: usize,
    pub repeated_header: bool,
    pub cannot_split: bool,
    pub grid_before: usize,
    pub overflow: Option<Overflow>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TableCellRegion {
    #[serde(flatten)]
    pub common: RegionCommon,
    pub source: SourceRef,
    pub fragment: PageFragment,
    pub row_index: usize,
    pub column_index: usize,
    pub grid_span: usize,
    pub row_span: usize,
    pub content_bbox_pt: Rect,
    pub padding_pt: EdgeInsets,
    pub vertical_alignment: CellVerticalAlignment,
    pub text_direction: TextDirection,
    pub borders: BoxBorders,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Fill>,
    pub overflow: Option<Overflow>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TableAlignment {
    Start,
    Center,
    Right,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TableLayoutMode {
    AutoFit,
    Fixed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CellVerticalAlignment {
    Top,
    Center,
    Bottom,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ImageRegion {
    #[serde(flatten)]
    pub common: RegionCommon,
    pub source: SourceRef,
    pub relationship: RelationshipRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intrinsic_size_px: Option<Size>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop: Option<NormalizedInsets>,
    pub opacity: f32,
    pub placement: ObjectPlacement,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ShapeRegion {
    #[serde(flatten)]
    pub common: RegionCommon,
    pub source: SourceRef,
    pub path: PathGeometry,
    pub fill: Fill,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
    pub opacity: f32,
    pub placement: ObjectPlacement,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RelationshipRef {
    pub source_part: String,
    pub id: String,
    pub target_part: String,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct NormalizedInsets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ShapeGeometry {
    Preset { name: String },
    Custom { path: PathGeometry },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ObjectPlacement {
    Inline,
    Floating { placement: FloatingPlacement },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FloatingPlacement {
    pub horizontal_anchor: AnchorReference,
    pub vertical_anchor: AnchorReference,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizontal_alignment: Option<AnchorAlignment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertical_alignment: Option<AnchorAlignment>,
    pub offset_pt: Point,
    pub wrap: WrapMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap_polygon: Option<PathGeometry>,
    pub distance_from_text_pt: EdgeInsets,
    pub z_order: i32,
    pub behind_text: bool,
    pub allow_overlap: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnchorReference {
    Page,
    Margin,
    Column,
    Paragraph,
    Character,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnchorAlignment {
    Start,
    Center,
    End,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WrapSide {
    Both,
    Left,
    Right,
    Largest,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WrapMode {
    None,
    Square { side: WrapSide },
    Tight { side: WrapSide },
    Through { side: WrapSide },
    TopAndBottom,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RgbaColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: f32,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BoxBorders {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top: Option<Border>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right: Option<Border>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bottom: Option<Border>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<Border>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Border {
    pub style: BorderStyle,
    pub color: RgbaColor,
    pub width_pt: f32,
    pub space_pt: f32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BorderStyle {
    None,
    Single,
    Double,
    Dotted,
    Dashed,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Fill {
    None,
    Solid {
        color: RgbaColor,
    },
    Gradient {
        stops: Vec<GradientStop>,
        angle_degrees: f32,
    },
    Image {
        relationship: RelationshipRef,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        crop: Option<NormalizedInsets>,
    },
    Pattern {
        preset: String,
        foreground: RgbaColor,
        background: RgbaColor,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GradientStop {
    pub position: f32,
    pub color: RgbaColor,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Stroke {
    pub color: RgbaColor,
    pub width_pt: f32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dash_pattern_pt: Vec<f32>,
    pub cap: LineCap,
    pub join: LineJoin,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LineCap {
    Butt,
    Round,
    Square,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Overflow {
    pub horizontal: bool,
    pub vertical: bool,
    pub clipped: bool,
    pub past_content_area_pt: f32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationErrors(pub Vec<ValidationError>);
impl fmt::Display for ValidationErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, e) in self.0.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{}: {}", e.path, e.message)?;
        }
        Ok(())
    }
}
impl std::error::Error for ValidationErrors {}

pub fn validate(document: &DocumentLayout) -> Result<(), ValidationErrors> {
    let mut e = Vec::new();
    if let Err(x) = document.compatibility() {
        push(&mut e, "$", x.to_string());
    }
    nonempty(&mut e, "generator.name", &document.generator.name);
    nonempty(&mut e, "generator.version", &document.generator.version);
    nonempty(&mut e, "source.path", &document.source.path);
    let mut sections = HashMap::new();
    for (i, s) in document.sections.iter().enumerate() {
        if s.index != i {
            push(
                &mut e,
                format!("sections[{i}].index"),
                format!("must be {i}"),
            );
        }
        if s.page_range[0] > s.page_range[1] || s.page_range[1] > document.pages.len() {
            push(
                &mut e,
                format!("sections[{i}].page_range"),
                "must be an ordered half-open range within pages",
            );
        }
        if sections.insert(s.id.clone(), i).is_some() {
            push(&mut e, format!("sections[{i}].id"), "must be unique");
        }
        source(&mut e, &format!("sections[{i}].source"), &s.source);
    }
    for (pi, page) in document.pages.iter().enumerate() {
        let pp = format!("pages[{pi}]");
        if page.index != pi {
            push(&mut e, format!("{pp}.index"), format!("must be {pi}"));
        }
        positive(&mut e, &format!("{pp}.size_pt.width"), page.size_pt.width);
        positive(&mut e, &format!("{pp}.size_pt.height"), page.size_pt.height);
        for (gi, geometry) in page.section_geometries.iter().enumerate() {
            let gp = format!("{pp}.section_geometries[{gi}]");
            rect(
                &mut e,
                &format!("{gp}.usable_content_bbox_pt"),
                geometry.usable_content_bbox_pt,
            );
            finite(&mut e, &format!("{gp}.gutter_pt"), geometry.gutter_pt);
            match sections.get(&geometry.section_id) {
                Some(si)
                    if document.sections[*si].page_range[0] <= pi
                        && pi < document.sections[*si].page_range[1] => {}
                _ => push(
                    &mut e,
                    format!("{gp}.section_id"),
                    "must reference a section whose page range contains this page",
                ),
            }
            for (ci, column) in geometry.columns.iter().enumerate() {
                if column.index != ci {
                    push(
                        &mut e,
                        format!("{gp}.columns[{ci}].index"),
                        format!("must be {ci}"),
                    );
                }
                rect(
                    &mut e,
                    &format!("{gp}.columns[{ci}].bbox_pt"),
                    column.bbox_pt,
                );
            }
        }
        let mut ids = HashMap::new();
        for (ri, region) in page.regions.iter().enumerate() {
            let rp = format!("{pp}.regions[{ri}]");
            let c = region.common();
            nonempty(&mut e, &format!("{rp}.id"), &c.id.0);
            if ids.insert(c.id.clone(), ri).is_some() {
                push(&mut e, format!("{rp}.id"), "must be unique on the page");
            }
            geometry(&mut e, &format!("{rp}.geometry"), &c.geometry);
            validate_region(
                &mut e,
                &rp,
                region,
                document
                    .layout_environment
                    .as_ref()
                    .map_or(0, |x| x.fonts.len()),
            );
        }
        let mut orders = HashSet::new();
        for (ri, region) in page.regions.iter().enumerate() {
            let c = region.common();
            for (field, link) in [("parent_id", &c.parent_id), ("owner_id", &c.owner_id)] {
                if let Some(id) = link {
                    match ids.get(id) {
                        Some(target) if *target < ri => {}
                        Some(_) => push(
                            &mut e,
                            format!("{pp}.regions[{ri}].{field}"),
                            "must reference an earlier region on the same page",
                        ),
                        None => push(
                            &mut e,
                            format!("{pp}.regions[{ri}].{field}"),
                            "references an unknown region",
                        ),
                    }
                }
            }
            if let Some(order) = c.reading_order {
                let key = (c.parent_id.clone(), order);
                if !orders.insert(key) {
                    push(
                        &mut e,
                        format!("{pp}.regions[{ri}].reading_order"),
                        "must be unique among siblings",
                    );
                }
            }
            if let Some(parent) = &c.parent_id {
                let pk = page.regions[*ids.get(parent).unwrap_or(&0)].kind_name();
                let valid = matches!(
                    (pk, region.kind_name()),
                    ("story", "paragraph" | "table" | "image" | "shape")
                        | ("table", "table_row")
                        | ("table_row", "table_cell")
                        | ("table_cell", "paragraph" | "table" | "image" | "shape")
                        | ("shape", "story")
                );
                if !valid {
                    push(
                        &mut e,
                        format!("{pp}.regions[{ri}].parent_id"),
                        format!("{pk} cannot contain {}", region.kind_name()),
                    );
                }
            } else if !matches!(region, Region::Story(_)) {
                push(
                    &mut e,
                    format!("{pp}.regions[{ri}].parent_id"),
                    "only story regions may be composition roots",
                );
            }
        }
    }
    if e.is_empty() {
        Ok(())
    } else {
        Err(ValidationErrors(e))
    }
}

fn validate_region(e: &mut Vec<ValidationError>, p: &str, r: &Region, _font_count: usize) {
    match r {
        Region::Story(x) => source(e, &format!("{p}.source"), &x.source),
        Region::Paragraph(x) => {
            source(e, &format!("{p}.source"), &x.source);
            fragment(e, p, x.fragment);
            rect(e, &format!("{p}.frame_bbox_pt"), x.frame_bbox_pt);
            if let Some(bounds) = x.content_bbox_pt {
                rect(e, &format!("{p}.content_bbox_pt"), bounds);
            }
            ranges(
                e,
                p,
                &x.text,
                &x.lines
                    .iter()
                    .map(|v| v.range_utf8)
                    .chain(x.runs.iter().map(|v| v.range_utf8))
                    .chain(x.content.iter().map(|v| v.range_utf8))
                    .collect::<Vec<_>>(),
            );
            for (i, l) in x.lines.iter().enumerate() {
                rect(e, &format!("{p}.lines[{i}].bbox_pt"), l.bbox_pt);
                finite(e, &format!("{p}.lines[{i}].baseline_y_pt"), l.baseline_y_pt);
                if l.baseline_y_pt < l.bbox_pt.y || l.baseline_y_pt > l.bbox_pt.bottom() {
                    push(
                        e,
                        format!("{p}.lines[{i}].baseline_y_pt"),
                        "must lie within line bounds",
                    );
                }
                if l.ascent_pt < 0.0 || l.descent_pt < 0.0 {
                    push(
                        e,
                        format!("{p}.lines[{i}]"),
                        "ascent and descent must be non-negative",
                    );
                }
            }
            for (i, run) in x.runs.iter().enumerate() {
                positive(
                    e,
                    &format!("{p}.runs[{i}].typography.size_pt"),
                    run.typography.size_pt,
                );
                nonempty(
                    e,
                    &format!("{p}.runs[{i}].typography.font.family"),
                    &run.typography.font.family,
                );
                color(
                    e,
                    &format!("{p}.runs[{i}].typography.color"),
                    run.typography.color,
                );
            }
            overflow(e, p, x.overflow);
        }
        Region::Table(x) => {
            source(e, &format!("{p}.source"), &x.source);
            fragment(e, p, x.fragment);
            rect(e, &format!("{p}.content_bbox_pt"), x.content_bbox_pt);
            overflow(e, p, x.overflow);
        }
        Region::TableRow(x) => {
            source(e, &format!("{p}.source"), &x.source);
            fragment(e, p, x.fragment);
            overflow(e, p, x.overflow);
        }
        Region::TableCell(x) => {
            source(e, &format!("{p}.source"), &x.source);
            fragment(e, p, x.fragment);
            if x.grid_span == 0 {
                push(e, format!("{p}.grid_span"), "must be greater than zero");
            }
            if x.row_span == 0 {
                push(e, format!("{p}.row_span"), "must be greater than zero");
            }
            overflow(e, p, x.overflow);
        }
        Region::Image(x) => {
            source(e, &format!("{p}.source"), &x.source);
            unit(e, &format!("{p}.opacity"), x.opacity);
            if let Some(c) = x.crop {
                insets_unit(e, &format!("{p}.crop"), c);
            }
        }
        Region::Shape(x) => {
            source(e, &format!("{p}.source"), &x.source);
            unit(e, &format!("{p}.opacity"), x.opacity);
        }
    }
}
fn fragment(e: &mut Vec<ValidationError>, p: &str, f: PageFragment) {
    if f.count == 0 || f.index >= f.count {
        push(
            e,
            format!("{p}.fragment"),
            "index must be less than a non-zero count",
        );
    }
    if f.continued_from_previous != (f.index > 0) || f.continues_on_next != (f.index + 1 < f.count)
    {
        push(
            e,
            format!("{p}.fragment"),
            "continuation flags must agree with index and count",
        );
    }
}
fn ranges(e: &mut Vec<ValidationError>, p: &str, text: &str, rs: &[[usize; 2]]) {
    for (i, [a, b]) in rs.iter().copied().enumerate() {
        if a > b || b > text.len() || !text.is_char_boundary(a) || !text.is_char_boundary(b) {
            push(
                e,
                format!("{p}.ranges[{i}]"),
                "must be an ordered UTF-8 range within paragraph text",
            );
        }
    }
}
fn geometry(e: &mut Vec<ValidationError>, p: &str, g: &RegionGeometry) {
    rect(e, &format!("{p}.bbox_pt"), g.bbox_pt);
    rect(e, &format!("{p}.local_bbox_pt"), g.local_bbox_pt);
    let t = g.local_to_parent;
    for (n, v) in [
        ("m11", t.m11),
        ("m12", t.m12),
        ("m21", t.m21),
        ("m22", t.m22),
        ("dx", t.dx),
        ("dy", t.dy),
    ] {
        finite(e, &format!("{p}.local_to_parent.{n}"), v)
    }
    let determinant = t.m11 * t.m22 - t.m12 * t.m21;
    if determinant == 0.0 {
        push(e, format!("{p}.local_to_parent"), "must be invertible");
    }
}
fn source(e: &mut Vec<ValidationError>, p: &str, s: &SourceRef) {
    if crate::source_identity::validate_part_name(&s.part).is_err() {
        push(
            e,
            format!("{p}.part"),
            "must be a canonical package-relative OPC part name",
        )
    }
    nonempty(e, &format!("{p}.id"), &s.id)
}
fn rect(e: &mut Vec<ValidationError>, p: &str, r: Rect) {
    for (n, v) in [
        ("x", r.x),
        ("y", r.y),
        ("width", r.width),
        ("height", r.height),
    ] {
        if !v.is_finite() || ((n == "width" || n == "height") && v < 0.0) {
            push(
                e,
                format!("{p}.{n}"),
                "must be finite and dimensions non-negative",
            )
        }
    }
}
fn overflow(e: &mut Vec<ValidationError>, p: &str, o: Option<Overflow>) {
    if let Some(o) = o
        && (!o.past_content_area_pt.is_finite() || o.past_content_area_pt < 0.0)
    {
        push(
            e,
            format!("{p}.overflow.past_content_area_pt"),
            "must be finite and non-negative",
        )
    }
}
fn color(e: &mut Vec<ValidationError>, p: &str, c: RgbaColor) {
    unit(e, &format!("{p}.alpha"), c.alpha)
}
fn insets_unit(e: &mut Vec<ValidationError>, p: &str, c: NormalizedInsets) {
    for (n, v) in [
        ("top", c.top),
        ("right", c.right),
        ("bottom", c.bottom),
        ("left", c.left),
    ] {
        unit(e, &format!("{p}.{n}"), v)
    }
}
fn unit(e: &mut Vec<ValidationError>, p: &str, v: f32) {
    if !v.is_finite() || !(0.0..=1.0).contains(&v) {
        push(e, p, "must be in 0..=1")
    }
}
fn positive(e: &mut Vec<ValidationError>, p: &str, v: f32) {
    if !v.is_finite() || v <= 0.0 {
        push(e, p, "must be finite and greater than zero")
    }
}
fn finite(e: &mut Vec<ValidationError>, p: &str, v: f32) {
    if !v.is_finite() {
        push(e, p, "must be finite")
    }
}
fn nonempty(e: &mut Vec<ValidationError>, p: &str, v: &str) {
    if v.trim().is_empty() {
        push(e, p, "must not be empty")
    }
}
fn push(e: &mut Vec<ValidationError>, p: impl Into<String>, m: impl Into<String>) {
    e.push(ValidationError {
        path: p.into(),
        message: m.into(),
    })
}

#[derive(Debug)]
pub enum CanonicalJsonError {
    Validation(ValidationErrors),
    Serialization(serde_json::Error),
    Limit(LimitError),
}
impl fmt::Display for CanonicalJsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(e) => e.fmt(f),
            Self::Serialization(e) => e.fmt(f),
            Self::Limit(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for CanonicalJsonError {}
pub fn to_canonical_json(d: &DocumentLayout) -> Result<Vec<u8>, CanonicalJsonError> {
    to_canonical_json_with_limits(d, &ProcessingLimits::default())
}
pub fn to_canonical_json_with_limits(
    d: &DocumentLayout,
    l: &ProcessingLimits,
) -> Result<Vec<u8>, CanonicalJsonError> {
    d.validate().map_err(CanonicalJsonError::Validation)?;
    d.check_limits(l).map_err(CanonicalJsonError::Limit)?;
    let mut canonical = d.clone();
    if let Some(environment) = &mut canonical.layout_environment {
        environment.fonts.sort_by(|left, right| {
            (&left.family, &left.postscript_name, left.face_index).cmp(&(
                &right.family,
                &right.postscript_name,
                right.face_index,
            ))
        });
        environment.fonts.dedup();
        environment.font_substitutions.sort_by(|left, right| {
            (
                &left.requested_family,
                &left.resolved_family,
                &left.resolved_postscript_name,
            )
                .cmp(&(
                    &right.requested_family,
                    &right.resolved_family,
                    &right.resolved_postscript_name,
                ))
        });
        environment.font_substitutions.dedup();
    }
    let mut v = serde_json::to_value(canonical).map_err(CanonicalJsonError::Serialization)?;
    canonicalize_value(&mut v);
    sort_value(&mut v);
    let mut w = CappedWriter {
        bytes: Vec::new(),
        limit: l.max_generated_ir_bytes,
        exceeded: false,
    };
    if let Err(x) = serde_json::to_writer(&mut w, &v) {
        if w.exceeded {
            return Err(CanonicalJsonError::Limit(LimitError {
                resource: "generated IR bytes",
                observed: l.max_generated_ir_bytes.saturating_add(1),
                limit: l.max_generated_ir_bytes,
            }));
        }
        return Err(CanonicalJsonError::Serialization(x));
    }
    Ok(w.bytes)
}
fn canonicalize_value(v: &mut Value) {
    match v {
        Value::Number(n) if n.is_f64() => {
            if let Some(x) = n.as_f64() {
                let x = (x * 1_000.0).round_ties_even() / 1_000.0;
                *v = serde_json::json!(if x == 0.0 { 0.0 } else { x });
            }
        }
        Value::Array(a) => a.iter_mut().for_each(canonicalize_value),
        Value::Object(o) => o.values_mut().for_each(canonicalize_value),
        _ => {}
    }
}
fn sort_value(v: &mut Value) {
    match v {
        Value::Object(o) => {
            let old = std::mem::take(o);
            let mut keys = old.into_iter().collect::<Vec<_>>();
            keys.sort_by(|a, b| a.0.cmp(&b.0));
            for (k, mut v) in keys {
                sort_value(&mut v);
                o.insert(k, v);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(sort_value),
        _ => {}
    }
}
struct CappedWriter {
    bytes: Vec<u8>,
    limit: u64,
    exceeded: bool,
}
impl Write for CappedWriter {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        if (self.bytes.len() as u64).saturating_add(b.len() as u64) > self.limit {
            self.exceeded = true;
            return Err(std::io::Error::other("generated IR byte limit exceeded"));
        }
        self.bytes.extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn count_value(v: &Value, s: &mut IrStats) {
    match v {
        Value::String(x) => s.string_bytes = s.string_bytes.saturating_add(x.len() as u64),
        Value::Array(a) => {
            s.elements = s.elements.saturating_add(a.len() as u64);
            for x in a {
                count_value(x, s)
            }
        }
        Value::Object(o) => {
            s.elements = s.elements.saturating_add(o.len() as u64);
            for (k, v) in o {
                s.string_bytes = s.string_bytes.saturating_add(k.len() as u64);
                count_value(v, s)
            }
        }
        _ => {}
    }
}
fn check_limit(resource: &'static str, observed: u64, limit: u64) -> Result<(), LimitError> {
    if observed > limit {
        Err(LimitError {
            resource,
            observed,
            limit,
        })
    } else {
        Ok(())
    }
}

pub fn json_schema() -> schemars::Schema {
    schemars::schema_for!(DocumentLayout)
}
