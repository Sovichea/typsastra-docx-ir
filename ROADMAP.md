# Roadmap

This roadmap tracks the work required to make `typsastra-docx-ir` a standalone, source-aware DOCX layout inspection tool.

The intended workflow is:

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

The planned workspace boundary is:

```text
typsastra-docx-ir/
├── src/                         # core IR, schema, validation, summary, and diff
├── crates/
│   ├── typsastra-docx-ir-dxpdf/ # DOCX layout producer backed by dxpdf and Skia
│   └── typsastra-docx-ir-cli/   # standalone command-line application
├── schema/
├── examples/
└── fixtures/
```

The core owns the public data contract. Producer-specific paint commands and internal renderer types must not leak into the canonical IR.

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
- [ ] Define deterministic numeric rounding and serialization rules.
- [ ] Record layout-engine, platform, font, and font-substitution metadata.
- [ ] Add compatibility fixtures for unknown fields and supported format versions.
- [ ] Add input limits for document size, ZIP expansion, element counts, nesting, and generated IR size.

## Milestone 2: Durable source identity

- [ ] Parse and preserve WordprocessingML `w14:paraId` values.
- [ ] Generate deterministic structural-path IDs when native IDs are absent.
- [ ] Qualify every source ID with its OPC part, such as `word/document.xml` or `word/footnotes.xml`.
- [ ] Carry provenance structurally through parse, style resolution, pagination, and IR conversion.
- [ ] Keep one source ID stable when a paragraph splits across pages.
- [ ] Assign segment indexes and counts after pagination.
- [ ] Add fixtures proving identity survives text and style edits.
- [ ] Document expected identity changes after insertions, deletions, and node replacement.

Do not inject bookmarks or marker text to recover identity: those techniques can change the layout being measured.

## Milestone 3: Standalone DOCX inspection

- [ ] Create `typsastra-docx-ir-dxpdf` as a separate producer adapter.
- [ ] Move or port source-aware paragraph collection from the dxpdf prototype.
- [ ] Convert producer output into canonical v1 types rather than exposing paint commands.
- [ ] Add `inspect <docx> -o <json>` to the CLI.
- [ ] Ensure inspection performs layout directly and never emits an intermediate PDF.
- [ ] Collect metadata only during IR inspection so normal PDF rendering does not retain duplicate line data.
- [ ] Return actionable errors for malformed packages, unsupported content, missing fonts, and layout failures.
- [ ] Add end-to-end tests that inspect DOCX fixtures and assert that no PDF is created.

## Milestone 4: Complete document structure

### Paragraphs and text

- [ ] Emit frame bounds, content bounds, line bounds, baselines, and overflow.
- [ ] Represent explicit breaks, tabs, fields, list labels, and empty paragraphs consistently.
- [ ] Cover columns, keep rules, widow/orphan behavior, and page/column breaks.

### Tables

- [ ] Add stable table, row, and cell source references.
- [ ] Emit table, row, cell, and cell-content bounds.
- [ ] Represent repeated headers, merged cells, grid spans, nested tables, and split rows.
- [ ] Report clipping and overflow at table and cell level.
- [ ] Add complex booktabs-style fixtures spanning multiple pages.

### Images and floating objects

- [ ] Add structural IDs and owner references for inline and floating objects.
- [ ] Emit intrinsic size, rendered bounds, crop, rotation, and source relationship.
- [ ] Represent anchors, offsets, alignment, wrapping mode, wrap polygon, z-order, and overlap behavior.
- [ ] Cover images in paragraphs, headers, footers, notes, text boxes, and table cells.

### Stories, notes, and sections

- [ ] Add headers, footers, footnotes, endnotes, comments, and text-box story regions.
- [ ] Link note references to note bodies and page placements.
- [ ] Emit section boundaries, page size, orientation, columns, margins, and usable content bounds.
- [ ] Apply odd/even mirrored margins and binding gutter to resolved page geometry.
- [ ] Cover different first-page and odd/even header/footer rules.

## Milestone 5: Layout-aware diff

- [ ] Add `diff before.json after.json`.
- [ ] Match regions primarily by OPC-qualified durable source ID.
- [ ] Compare page placement, segment count, line count, bounds, and overflow.
- [ ] Compare table, cell, image, note, and story-region geometry.
- [ ] Distinguish content changes, reflow, movement, resize, addition, and removal.
- [ ] Support concise human-readable output and stable machine-readable JSON.
- [ ] Define tolerances for insignificant floating-point differences.
- [ ] Return useful exit codes for CI layout-regression checks.

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

## v1 release criteria

A v1 release should not be declared until all of the following are true:

- [ ] A packaged CLI supports `inspect`, `validate`, `summary`, `diff`, and `schema`.
- [ ] The core library remains renderer-independent and Skia-free.
- [ ] Paragraphs, tables, images, headers, footers, footnotes, endnotes, sections, and binding geometry have source-aware regions.
- [ ] Durable IDs support meaningful before/after comparison across ordinary edits.
- [ ] Inspection creates canonical IR without generating or parsing PDF output.
- [ ] Schema compatibility and deterministic-output policies are documented and tested.
- [ ] Native binaries pass end-to-end tests on clean Windows, Linux, and macOS environments.
- [ ] Security limits and malformed/untrusted DOCX behavior are documented and tested.

## Non-goals for v1

- Pixel-perfect visual reconstruction from the IR.
- OCR or extraction from PDF files.
- Exposing every glyph, paint command, or renderer-internal object.
- Replacing dxpdf's layout engine or removing Skia from the producer.
- Guaranteeing identical pagination across platforms when fonts or layout-engine builds differ.
