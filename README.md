# Typsastra DOCX IR

A source-aware intermediate representation of paginated DOCX layout.

Typsastra DOCX IR is designed to map editable WordprocessingML elements to inspectable layout without generating or rereading a PDF. The goal is engine-populated geometry and resolved appearance for AI document editing, layout regression testing, template evaluation, and renderer interoperability.

> Editable DOCX structure, resolved into inspectable page geometry.

## Status

This repository is an early v0.1 foundation with a `1.0` format contract. The Rust types establish the renderer-independent boundary. An initial engine-populated body-paragraph producer is now implemented as `typsastra-inspect` in the sibling `typsastra-docx` workspace; standalone distribution and comprehensive measurements remain pending.

The current contract models pages, durable OPC-qualified paragraph identities, source-aware paragraph segments, fitted-line UTF-8 ranges, images, overflow, and diagnostics. Optional measurement coverage declarations are supported. Resolved appearance, composition hierarchy, tables, shapes, and story-region extensions remain pending.

## Goals

- preserve durable references to editable OOXML nodes
- expose pagination, fitted lines, bounds, overflow, tables, and anchored media
- remain independent of any particular layout or paint backend
- provide compact, versioned JSON suitable for automated tools and AI agents
- inspect layout without PDF generation, rasterization, or OCR

## CLI

Validate IR structure and semantic invariants, not document design or measurement completeness:

```bash
cargo run -- validate examples/minimal.docx-ir.json
```

Print a compact summary of recorded data (counts do not establish completeness):

```bash
cargo run -- summary examples/minimal.docx-ir.json
```

Print or regenerate the JSON Schema:

```bash
cargo run -- schema
cargo run -- schema --output schema/typsastra-docx-ir-v1.schema.json
```

## Library

```rust
use typsastra_docx_ir::{DocumentLayout, GeneratorInfo, SourceInfo};

let layout = DocumentLayout::new(
    GeneratorInfo {
        name: "typsastra-dxpdf".into(),
        version: "0.5.1".into(),
    },
    SourceInfo {
        path: "book.docx".into(),
        byte_length: 42,
    },
    Vec::new(),
);

layout.validate()?;
let canonical_json = typsastra_docx_ir::to_canonical_json(&layout)?;
```

The empty layout above can be valid without containing any measurements. Valid IR does not mean measured content, complete coverage, or acceptable design.

## Format conventions

- coordinates and dimensions are PDF points, with 72 points per inch
- the origin is the page's top-left corner; x increases right and y increases down
- page indexes and paragraph segment indexes are zero-based
- rectangles are represented by x, y, width, and height
- line ranges are half-open UTF-8 byte ranges into their paragraph text
- source references always include the canonical package-relative OPC part containing the editable node
- paragraph identities prefer namespace-resolved `w14:paraId` values and otherwise use versioned structural paths
- all page segments of one paragraph occurrence retain one source identity and receive indexes only after pagination
- consumers must ignore unknown fields within a compatible major version
- version strings use strict, unpadded `major.minor` decimal syntax with each component in the `u16` range; this release reads supported `1.x` versions
- on the JSON wire, adding optional fields is compatible within v1; removing fields or changing meanings requires a new major version
- the existing `Region` enum cannot accept new table/shape variants; adding them requires a major version or a deliberately designed forward-compatible representation first
- unknown fields are discarded when JSON is deserialized into the typed Rust model

### Coverage declarations

The optional `DocumentLayout.coverage` field is additive; the format version stays `1.0`. Its dimensions are `pagination`, `text_geometry`, `resolved_typography`, `table_geometry`, `drawing_appearance`, `composition_hierarchy`, and `overflow`.

Each dimension uses `unknown` (default), `partial`, `complete`, or `absent`. Missing coverage or dimensions mean `unknown`. `complete` declares measurements for all applicable content; `absent` means measurements were not captured, not that content is missing. There is no `not_applicable` state.

These are global producer declarations, not independently verified evidence. Unknown coverage is never a pass. The CLI distinguishes recorded counts from declared coverage and unknown completeness; `validate` checks IR invariants, not design quality. An empty example passing validation is not evidence of measurement.

### Durable source identity

`extract_paragraph_identities` scans any WordprocessingML story part without depending on a renderer. It resolves native paragraph IDs by namespace URI, canonicalizes them to uppercase, and generates prefix-independent physical XML paths when native IDs are absent. The extractor charges one ZIP-entry budget and every expanded input byte to `PackageBudget` before parsing. `Sourced<T>` preserves the resulting `SourceRef` through processing stages, and `finish_paginated_paragraph` assigns segment indexes after all page fragments are known.

`extract_paragraph_identities_with_spans` additionally exposes exact opening-tag byte ranges for an in-memory parser bridge. These spans refer to the supplied XML bytes, are not durable IDs, and avoid joining XML and layout by ordinal. Both extraction APIs share identity rules and budget accounting.

The complete identity is `(part, identity, id)`, never the bare ID. Structural paths survive text and style changes but can change after structural edits. See [Source identity](docs/source-identity.md) for the path grammar and the exact insertion, deletion, movement, and replacement behavior.

### Deterministic serialization

`to_canonical_json` is the normative producer serialization path. Canonical JSON is compact UTF-8 with no trailing newline. Object keys are sorted recursively by their UTF-8 bytes, while semantically meaningful array order is preserved.

All point measurements are rounded at serialization time to the nearest `0.001 pt`, with ties rounded to the nearest even value. Non-finite values are invalid, and values that round to zero are emitted as positive zero. Readers may accept additional finite precision; producers should use canonical serialization so insignificant floating-point noise does not change output.

Font and font-substitution records have no semantic order. Canonical serialization sorts them bytewise by their identity fields and removes exact duplicates.

### Layout environment

`generator` identifies the application that emitted the IR. Optional `layout_environment` metadata separately records the engine that performed pagination, the operating system and architecture, resolved font faces actually used during layout, and observed font substitutions.

Font paths and the machine's complete installed-font inventory are intentionally excluded. When available, `file_sha256` is the lowercase SHA-256 of the exact font file and `face_index` is the zero-based face within a collection. A missing `layout_environment` means the producer did not record this metadata; it does not imply a particular engine or platform.

### Resource limits

`ProcessingLimits` defines conservative defaults for source document bytes, ZIP entries and expansion, XML/JSON nesting, XML and IR element counts, aggregate strings, paragraph text, and generated IR bytes. `read_document` bounds raw JSON before deserialization, then checks typed element and string budgets before semantic validation. DOCX producers use `PackageBudget` while streaming package and XML input, and `to_canonical_json_with_limits` checks in-memory budgets before cloning and caps generated output while writing.

The JSON byte limit is the allocation boundary during deserialization; the finer element and string budgets bound subsequent processing rather than individual Serde allocations. The limits are security defaults rather than format maxima. Library callers may tune them for trusted, unusually large documents.

The canonical schema is `schema/typsastra-docx-ir-v1.schema.json`.

## Planned commands

```text
typsastra-docx-ir inspect book.docx
typsastra-docx-ir diff before.docx-ir.json after.docx-ir.json
```

Initial inspection is implemented by `typsastra-inspect` in the sibling `typsastra-docx` workspace. Run `cargo run -p typsastra-inspect -- inspect book.docx -o book.docx-ir.json` there. The producer pins a core Git revision, so a sibling checkout is not required. The output path must be new. The adapter captures actual pagination and body-paragraph geometry without an intermediate PDF and emits canonical IR with partial/absent coverage. It does not yet capture table-cell or repeated-story regions, appearance, or complete overflow. This is a development CLI, not the finished standalone distribution. The core IR, schema, validation, summary, and future diff/evaluation tooling remain independent of Skia and dxpdf.

## Roadmap

- [x] Establish the renderer-independent Rust IR, schema, validation, and summary tooling.
- [x] Add durable OPC-qualified source IDs with `w14:paraId` support and deterministic fallbacks.
- [x] Integrate initial engine-populated body-paragraph IR inspection in the sibling workspace without PDF output.
- [x] Add optional coverage declarations and coverage-aware CLI reporting.
- [ ] Capture resolved run, paragraph, table, image, and shape appearance, including baselines, clips, and transforms.
- [ ] Preserve composition hierarchy, reading order, and role provenance alongside source identities.
- [ ] Choose a compatible representation or major version before adding table/shape regions; cover stories, notes, sections, and binding geometry.
- [ ] Add source-linked diffs and separate objective checks from advisory evaluation, with unassessable outcomes when evidence is insufficient.
- [ ] Verify producer determinism and harden end-to-end processing of untrusted DOCX packages.
- [ ] Ship tested Windows, Linux, and macOS binaries that require no external PDF workflow.

See [ROADMAP.md](ROADMAP.md) for the detailed checklist, architecture boundaries, and inspection release criteria.

## License

MIT
