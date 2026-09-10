# Format v2 contract

Format `2.0` adds the composition and resolved-appearance contract required by Milestone 4. The Rust API is under `typsastra_docx_ir::v2`; unqualified crate types remain format v1.

## Compatibility

Both versions use `format: "typsastra-docx-ir"`. `read_document` remains a v1-only API, while `read_any_document` reads the envelope and returns `AnyDocumentLayout::V1` or `AnyDocumentLayout::V2` before typed deserialization. Consumers must ignore unknown object fields in a compatible major version.

A `2.x` minor may add optional object fields. It must not add required fields, change existing meanings, or add variants to closed enums such as `Region`, `StoryKind`, `SemanticRole`, `Fill`, or `WrapMode`. Those changes require another major version unless a deliberately open representation is introduced first.

## Composition

Each page contains a flat list of typed regions. `RegionCommon.id` identifies one laid-out instance and is unique across the document. It is not an editable source identity.

`parent_id` defines the composition tree:

```text
story
├── paragraph
│   ├── image
│   └── shape
├── table
│   └── table_row
│       └── table_cell
│           ├── paragraph
│           └── nested table
├── image
└── shape
    └── text_box story
```

`owner_id` records an anchoring or ownership relationship when it differs from the composition parent. Parent and owner references must resolve, parent references must be acyclic, and non-story regions must have a parent.

`reading_order` is zero-based among regions with the same parent. When supplied, measured sibling orders are unique and contiguous from zero. Paint order and `z_order` are separate facts and must not be inferred from reading order.

## Sources, occurrences, and fragments

`SourceRef` remains the complete tuple `(part, identity, id)`. V2 supports paragraph IDs, relationship IDs, Word IDs, drawing IDs, and generated `path-v1` structural locations. A relationship to image data is represented separately by `RelationshipRef`; it does not replace the structural source of the drawing object.

A repeated header or footer gets a distinct page-local story region ID and occurrence while retaining the same source reference. Split paragraphs, tables, rows, and cells use `PageFragment`. Fragment indexes are zero-based, counts are total occurrence counts, continuation flags agree with index/count, and continuation fragments occur on later physical pages.

## Role provenance

A role is optional measured metadata. `RoleAssignment` stores both `SemanticRole` and one of:

- `source_declared`, including the declaration and its source reference
- `inferred`, including the producer rule

Consumers must not treat inferred roles as source declarations. Missing role metadata means unknown, not body text or decorative content.

## Coordinates, transforms, and clips

All lengths are points. Page coordinates retain the top-left origin with x increasing right and y increasing down.

Every region has:

- `geometry.bbox_pt`: its axis-aligned envelope in page coordinates
- `geometry.local_bbox_pt`: bounds in its local coordinate system
- `geometry.local_to_parent`: the affine mapping from local to parent coordinates
- optional `geometry.clip`, expressed in local coordinates

Transforms compose through `parent_id` until page space is reached. The six affine values map a local point as:

```text
x' = m11*x + m21*y + dx
y' = m12*x + m22*y + dy
```

Transforms must be finite and invertible. Clips may be rectangles or renderer-independent paths. Shape paths contain move, line, cubic, and close commands; renderer paint commands and backend objects are not part of the IR.

## Paragraphs

Paragraphs store logical rendered text once. Lines, resolved runs, fields, list labels, tabs, breaks, and other inline records use half-open UTF-8 byte ranges into that text. Zero-width structural records may use `[offset, offset]`. Ranges must be ordered and fall on code-point boundaries.

Line records include bounds, baseline, ascent, and descent. Resolved runs include the actual font face, size, color, emphasis, decorations, spacing, horizontal scale, and baseline shift. Resolved paragraph style records alignment, direction, spacing, indentation, line spacing, borders, shading, keep rules, widow control, and page-break-before.

Field results and generated list labels are included in paragraph text and distinguished by inline semantics. Empty paragraphs remain explicit paragraph regions with empty text; producers must not omit them merely because they have no characters.

## Tables

Tables, rows, and cells are independent regions with source references and composition parents. Their contract includes page fragments, repeated headers, split rows, grid and row spans, nested tables, bounds, cell-content bounds, padding, alignment, borders, fills, and overflow. Missing overflow is an unknown measurement, not evidence that clipping did not occur.

## Images, shapes, and floating objects

Images retain structural source, media relationship, optional intrinsic pixel size, rendered geometry, normalized crop, opacity, and inline/floating placement. Shapes retain renderer-independent paths, resolved fill/stroke/opacity, and placement.

Floating placement records horizontal and vertical anchor references, optional alignment, offsets, wrap mode and polygon, text distances, z-order, behind-text behavior, and overlap behavior. Rotation, scale, reflection, and translation are represented by `local_to_parent` rather than duplicated scalar fields.

## Stories, notes, sections, and pages

Story kinds cover body, header, footer, footnote, endnote, comment, and text box. `linked_from` and inline target IDs connect note/comment bodies to references and objects to their regions.

Sections contain source identity, physical page range, break kind, and first/even-page rules. Each page records physical size and orientation plus an ordered `section_geometries` entry for every section occurring on it. This permits continuous sections to share a physical page while retaining their own resolved margins, gutter, usable content bounds, and columns. These entries are where a producer records the result after mirrored margins, binding gutter, page parity, and header/footer clearance are applied.

## Canonical JSON and coverage

`v2::to_canonical_json` validates, applies existing IR limits, sorts object keys recursively, preserves semantic array order, sorts and deduplicates environment font metadata, rounds finite floating-point values to three decimal places with ties-to-even, normalizes negative zero, and emits compact UTF-8 without a trailing newline.

Coverage remains producer-declared and uses the same seven dimensions as v1. Core support for a field does not establish producer coverage. Until a producer captures these records, its corresponding dimensions must remain `absent`, `partial`, or `unknown` as appropriate.
