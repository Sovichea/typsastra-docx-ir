# Roadmap

This roadmap tracks the work required to make `typsastra-docx-ir` a standalone, source-aware DOCX layout inspection tool.

The intended unified workflow is shown below. Initial inspection is available through `typsastra-inspect inspect` in the sibling workspace; unified distribution and `diff` remain pending:

```text
typsastra-docx-ir inspect book.docx -o book.docx-ir.json
typsastra-docx-ir validate book.docx-ir.json
typsastra-docx-ir summary book.docx-ir.json
typsastra-docx-ir diff before.docx-ir.json after.docx-ir.json
typsastra-docx-ir schema
```

## What standalone means

A release is standalone when a user can install one supported CLI distribution, inspect a DOCX directly, and consume the resulting IR without installing a PDF renderer, Microsoft Word, LibreOffice, Python, or a separate layout service.

The renderer-independent core must remain free of Skia and dxpdf dependencies. The `inspect` producer may use dxpdf and native Skia internally, and release executables may link or bundle those native components. Removing Skia is not a v1 requirement.

## Architecture

Reuse the existing sibling `typsastra-docx` workspace rather than create a producer repository or producer workspace here:

- `typsastra-docx-ir`: renderer-independent IR, schema, validation, summary, and future diff/evaluation tooling
- sibling `typsastra-docx`: existing layout engine and initial `typsastra-inspect` adapter/CLI workspace crate

The core stays independent of both Skia and dxpdf and owns the public data contract. The adapter must capture actual engine layout and resolved appearance without generating or rereading PDF. Producer-specific paint commands and internal renderer types must not leak into the canonical IR. Initial body-paragraph producer integration is implemented; complete measurement coverage and standalone packaging remain pending.

## Milestone 1: Stable core contract

- [x] Create the Rust library and CLI foundation.
- [x] Define versioned, renderer-independent document and page types.
- [x] Model source-aware paragraph segments, fitted lines, images, overflow, and diagnostics.
- [x] Store paragraph text once and represent lines with half-open UTF-8 byte ranges.
- [x] Document points, top-left coordinates, indexes, rectangles, and source references.
- [x] Generate the canonical v1 JSON Schema.
- [x] Add semantic validation with path-aware errors.
- [x] Add `validate`, `summary`, and `schema` commands.
- [x] Add a validated minimal example and schema-drift CI check.
- [x] Define deterministic numeric rounding and serialization rules.
- [x] Record layout-engine, platform, font, and font-substitution metadata.
- [x] Add compatibility fixtures for unknown fields and supported format versions.
- [x] Add input limits for document size, ZIP expansion, element counts, nesting, and generated IR size.

### Coverage declarations

- [x] Add optional `DocumentLayout.coverage` with `pagination`, `text_geometry`, `resolved_typography`, `table_geometry`, `drawing_appearance`, `composition_hierarchy`, and `overflow`.
- [x] Support `unknown` (default), `partial`, `complete`, and `absent`; missing coverage or dimensions mean `unknown`. Do not add `not_applicable`.
- [x] Keep format version `1.0`: coverage is an additive optional field, with schema and compatibility tests updated accordingly.
- [x] Distinguish recorded counts, declared coverage, and unknown completeness in CLI output.
- [x] Test and document that empty valid IR is not evidence of measurements or design quality.

Coverage is a global producer declaration, not independently verified. `complete` includes all applicable content; `absent` means measurements were not captured, not that content is missing. Unknown coverage must never become a pass. `validate` checks IR invariants, not design quality or whether the producer captured everything.

## Milestone 2: Durable source identity

- [x] Parse and preserve WordprocessingML `w14:paraId` values.
- [x] Generate deterministic structural-path IDs when native IDs are absent.
- [x] Qualify every source ID with its OPC part, such as `word/document.xml` or `word/footnotes.xml`.
- [x] Provide provenance-preserving core APIs from identity extraction through style resolution, pagination, and IR conversion.
- [x] Keep one source ID stable when a paragraph splits across pages.
- [x] Assign segment indexes and counts after pagination.
- [x] Add fixtures proving identity survives text and style edits.
- [x] Document expected identity changes after insertions, deletions, and node replacement.

Do not inject bookmarks or marker text to recover identity: those techniques can change the layout being measured.

## Milestone 3: Standalone DOCX inspection

- [x] Integrate `typsastra-inspect` as an adapter crate in the existing sibling `typsastra-docx` workspace (development CLI).
- [x] Adapt the prototype's opt-in body-paragraph collection with OPC-qualified identities through parsing, resolution, and pagination; do not export its traversal counters as durable identities.
- [x] Carry provenance structurally using exact XML-tag spans and an internal parser attribute bridge, not ordinal joins or paint commands.
- [x] Populate canonical body-paragraph IR with actual engine results rather than placeholders or paint commands.
- [x] Expose `typsastra-inspect inspect <docx> -o <json>` as a development CLI.
- [ ] Package the standalone CLI for distribution; the producer pins its core dependency to a Git revision.
- [x] Declare partial text geometry and absent unsupported measurements explicitly.
- [x] Ensure inspection performs layout directly and never emits an intermediate PDF.
- [x] Collect metadata only during IR inspection so normal PDF rendering does not retain duplicate line data.
- [ ] Return actionable errors for malformed packages, unsupported content, missing fonts, and layout failures.
- [x] Add end-to-end tests with in-memory DOCX fixtures, source-aware split paragraphs, collection parity, and CLI assertions that no PDF is created.
- [x] Export observed font-resolution substitutions and instrumented ignored-property reports as structured diagnostics, excluding preload-only requests and false alias substitutions.
- [ ] Extend diagnostic coverage to remaining unsupported paths and exact source locations where available; bound auxiliary XML and engine resource use.

Milestone 3 is in progress, not complete. The initial adapter exports body-flow paragraphs only; repeated-story and table-cell region transport remain Milestone 4 work. Full overflow is unknown (`overflow: null`), not a negative measurement.

## Milestone 4: Resolved appearance and composition

The existing `Region` enum cannot accept table/shape variants. Optional coverage does not change that constraint.

- [ ] Choose a major format version or deliberately design a forward-compatible representation before adding table/shape variants; do not silently extend the v1 enum.
- [ ] Preserve composition hierarchy and parent/owner relationships across pages, stories, paragraphs, tables, and drawings.
- [ ] Record reading order and role provenance, distinguishing source-declared roles from inferred roles.
- [ ] Capture clips and transforms with explicit coordinate relationships, not only axis-aligned bounds.

### Paragraphs and text

- [ ] Emit frame bounds, content bounds, line bounds, baselines, and overflow.
- [ ] Capture resolved run typography and appearance, including actual fonts, sizes, colors, and decorations.
- [ ] Capture resolved paragraph spacing, indentation, alignment, borders, and shading rather than only style references.
- [ ] Represent explicit breaks, tabs, fields, list labels, and empty paragraphs consistently.
- [ ] Cover columns, keep rules, widow/orphan behavior, and page/column breaks.

### Tables

- [ ] Add stable table, row, and cell source references.
- [ ] Emit table, row, cell, and cell-content bounds.
- [ ] Capture resolved table/cell borders, fills, padding, and alignment.
- [ ] Represent repeated headers, merged cells, grid spans, nested tables, and split rows.
- [ ] Report clipping and overflow at table and cell level.
- [ ] Add complex booktabs-style fixtures spanning multiple pages.

### Images, shapes, and floating objects

- [ ] Add structural IDs and owner references for inline and floating objects.
- [ ] Emit intrinsic size, rendered bounds, crop, rotation, and source relationship.
- [ ] Capture resolved image and shape appearance, including shape geometry, fills, strokes, opacity, clips, and transforms.
- [ ] Represent anchors, offsets, alignment, wrapping mode, wrap polygon, z-order, and overlap behavior.
- [ ] Cover images in paragraphs, headers, footers, notes, text boxes, and table cells.

### Stories, notes, and sections

- [ ] Add headers, footers, footnotes, endnotes, comments, and text-box story regions.
- [ ] Link note references to note bodies and page placements.
- [ ] Emit section boundaries, page size, orientation, columns, margins, and usable content bounds.
- [ ] Apply odd/even mirrored margins and binding gutter to resolved page geometry.
- [ ] Cover different first-page and odd/even header/footer rules.

## Milestone 5: Source-linked diff and evaluation

- [ ] Add `diff before.json after.json`.
- [ ] Match regions primarily by OPC-qualified durable source ID.
- [ ] Compare page placement, segment count, line count, bounds, and overflow.
- [ ] Compare table, cell, image, shape, note, and story-region geometry, resolved appearance, hierarchy, and reading order.
- [ ] Link changes to editable source references and before/after page locations; flag coverage gaps rather than treating unmeasured content as removed.
- [ ] Distinguish content changes, reflow, movement, resize, addition, and removal.
- [ ] Support concise human-readable output and stable machine-readable JSON.
- [ ] Define tolerances for insignificant floating-point differences.
- [ ] Return useful exit codes for CI layout-regression checks.

### Separate evaluator (pending)

- [ ] Keep evaluation separate from core `validate`; validity is not a design verdict.
- [ ] Separate objective, evidence-based checks from advisory design recommendations.
- [ ] Return explicit unassessable outcomes when measurements or coverage are insufficient; unknown must not count as pass.
- [ ] Attach source references, measured evidence, and role provenance to findings; do not treat producer declarations as independent verification.

## Milestone 6: Distribution and hardening

- [ ] Build release binaries for Windows, Linux, and macOS.
- [ ] Decide per-platform static linking or native-library bundling strategy for Skia.
- [ ] Publish checksums and a software bill of materials with each release.
- [ ] Test release artifacts on clean machines without development toolchains.
- [ ] Add representative font installation and substitution tests.
- [ ] Make output deterministic for identical inputs, fonts, engine version, and platform.
- [ ] Fuzz DOCX package parsing and IR validation boundaries.
- [ ] Document memory, CPU, temporary-file, and untrusted-input behavior.
- [ ] Add performance benchmarks for small, complex, and book-length documents.

## Inspection release criteria

These product milestones do not imply that all extensions fit format v1. Coverage stays additive at `1.0`; table/shape regions require the compatibility decision above. A complete inspection release remains pending until:

- [ ] A packaged CLI supports `inspect`, `validate`, `summary`, `diff`, and `schema`.
- [ ] The core library remains renderer-independent and Skia-free.
- [ ] Paragraphs, tables, images, headers, footers, footnotes, endnotes, sections, and binding geometry have source-aware regions.
- [ ] Durable IDs support meaningful before/after comparison across ordinary edits.
- [ ] Inspection creates engine-populated canonical IR without generating or parsing PDF output.
- [ ] Resolved appearance, composition hierarchy, reading order, role provenance, baselines, clips, and transforms have tested coverage.
- [ ] CLI reports recorded data without implying completeness; evaluation separates objective and advisory results and supports unassessable outcomes.
- [ ] Diffs link changes to editable sources and expose measurement gaps.
- [ ] Schema compatibility and deterministic-output policies are documented and tested.
- [ ] Native binaries pass end-to-end tests on clean Windows, Linux, and macOS environments.
- [ ] Security limits and malformed/untrusted DOCX behavior are documented and tested.

## Non-goals for the inspection release

- Pixel-perfect visual reconstruction from the IR.
- OCR or extraction from PDF files.
- Exposing every glyph, paint command, or renderer-internal object.
- Replacing dxpdf's layout engine or removing Skia from the producer.
- Guaranteeing identical pagination across platforms when fonts or layout-engine builds differ.
