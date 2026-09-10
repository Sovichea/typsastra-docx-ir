use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{CompatibilityError, FORMAT, FormatVersion};

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
    /// Producer-declared measurement coverage, not independent verification.
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

    pub fn validate(&self) -> Result<(), crate::ValidationErrors> {
        super::validation::validate(self)
    }
}

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
    #[default]
    Unknown,
    Absent,
    Partial,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fonts: Vec<FontInfo>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_sha256: Option<String>,
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

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct SourceRef {
    pub part: String,
    pub id: String,
    pub identity: IdentityKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IdentityKind {
    ParaId,
    RelationshipId,
    WordId,
    DrawingId,
    GeneratedPath,
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
    pub source: SourceRef,
    pub index: usize,
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
    /// Resolved geometry for every section occurring on this physical page.
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
pub struct RegionCommon {
    pub id: RegionId,
    pub parent_id: Option<RegionId>,
    pub owner_id: Option<RegionId>,
    pub reading_order: Option<usize>,
    pub role: Option<RoleAssignment>,
    pub geometry: RegionGeometry,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RoleAssignment {
    pub role: SemanticRole,
    pub provenance: RoleProvenance,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RoleProvenance {
    SourceDeclared {
        source: SourceRef,
        declaration: String,
    },
    Inferred {
        rule: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RegionGeometry {
    /// Axis-aligned envelope in page coordinates.
    pub bbox_pt: Rect,
    /// Bounds in the region's local coordinates.
    pub local_bbox_pt: Rect,
    /// Affine mapping from local coordinates into parent coordinates.
    pub local_to_parent: AffineTransform,
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

    pub fn determinant(self) -> f32 {
        self.m11 * self.m22 - self.m12 * self.m21
    }

    pub fn transform_point(self, point: Point) -> Point {
        Point {
            x: self.m11 * point.x + self.m21 * point.y + self.dx,
            y: self.m12 * point.x + self.m22 * point.y + self.dy,
        }
    }
}

impl Default for AffineTransform {
    fn default() -> Self {
        Self::identity()
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Point {
    pub x: f32,
    pub y: f32,
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
            Self::Story(value) => &value.common,
            Self::Paragraph(value) => &value.common,
            Self::Table(value) => &value.common,
            Self::TableRow(value) => &value.common,
            Self::TableCell(value) => &value.common,
            Self::Image(value) => &value.common,
            Self::Shape(value) => &value.common,
        }
    }

    pub fn source(&self) -> &SourceRef {
        match self {
            Self::Story(value) => &value.source,
            Self::Paragraph(value) => &value.source,
            Self::Table(value) => &value.source,
            Self::TableRow(value) => &value.source,
            Self::TableCell(value) => &value.source,
            Self::Image(value) => &value.source,
            Self::Shape(value) => &value.source,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StoryRegion {
    #[serde(flatten)]
    pub common: RegionCommon,
    pub source: SourceRef,
    pub story_kind: StoryKind,
    /// Zero-based occurrence for repeated stories such as headers and footers.
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
    /// Half-open UTF-8 byte range into `ParagraphRegion::text`.
    pub range_utf8: [usize; 2],
    pub bbox_pt: Rect,
    pub baseline_y_pt: f32,
    pub ascent_pt: f32,
    pub descent_pt: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ResolvedRun {
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
    pub underline: Option<TextDecoration>,
    pub strike: Option<TextDecoration>,
    pub letter_spacing_pt: f32,
    pub horizontal_scale: f32,
    pub baseline_shift_pt: f32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResolvedFont {
    pub family: String,
    pub postscript_name: Option<String>,
    pub face_index: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TextDecoration {
    pub style: DecorationStyle,
    pub color: RgbaColor,
    pub width_pt: Option<f32>,
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
    pub source: Option<SourceRef>,
    #[serde(flatten)]
    pub kind: InlineKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InlineKind {
    Text,
    Tab {
        alignment: Option<TabAlignment>,
    },
    Break {
        break_kind: BreakKind,
    },
    Field {
        instruction: String,
        result_range_utf8: [usize; 2],
    },
    ListLabel,
    NoteReference {
        target_story_id: RegionId,
    },
    CommentReference {
        target_story_id: RegionId,
    },
    Object {
        target_region_id: RegionId,
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
    pub grid_before: u32,
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
    pub grid_span: u32,
    pub row_span: u32,
    pub content_bbox_pt: Rect,
    pub padding_pt: EdgeInsets,
    pub vertical_alignment: CellVerticalAlignment,
    pub text_direction: TextDirection,
    pub borders: BoxBorders,
    pub fill: Option<Fill>,
    pub overflow: Option<Overflow>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TableAlignment {
    Start,
    Center,
    End,
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
    Both,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RelationshipRef {
    pub source_part: String,
    pub id: String,
    pub target_part: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ImageRegion {
    #[serde(flatten)]
    pub common: RegionCommon,
    pub source: SourceRef,
    pub relationship: RelationshipRef,
    pub intrinsic_size_px: Option<Size>,
    pub crop: Option<NormalizedInsets>,
    pub placement: ObjectPlacement,
    pub opacity: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ShapeRegion {
    #[serde(flatten)]
    pub common: RegionCommon,
    pub source: SourceRef,
    pub path: PathGeometry,
    pub fill: Fill,
    pub stroke: Option<Stroke>,
    pub opacity: f32,
    pub placement: ObjectPlacement,
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
    pub horizontal_alignment: Option<AnchorAlignment>,
    pub vertical_alignment: Option<AnchorAlignment>,
    pub offset_pt: Point,
    pub wrap: WrapMode,
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
    Inside,
    Outside,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WrapMode {
    None,
    Square { side: WrapSide },
    Tight { side: WrapSide },
    Through { side: WrapSide },
    TopAndBottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WrapSide {
    Both,
    Left,
    Right,
    Largest,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Fill {
    None,
    Solid {
        color: RgbaColor,
    },
    Gradient {
        gradient: GradientFill,
    },
    Image {
        relationship: RelationshipRef,
        crop: Option<NormalizedInsets>,
    },
    Pattern {
        foreground: RgbaColor,
        background: RgbaColor,
        preset: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GradientFill {
    pub kind: GradientKind,
    pub stops: Vec<GradientStop>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GradientKind {
    Linear { angle_degrees: f32 },
    Radial,
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
    Round,
    Bevel,
    Miter,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct NormalizedInsets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EdgeInsets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BoxBorders {
    pub top: Option<Border>,
    pub right: Option<Border>,
    pub bottom: Option<Border>,
    pub left: Option<Border>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
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

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RgbaColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: f32,
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    pub source: Option<SourceRef>,
    pub region_id: Option<RegionId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Error,
}
