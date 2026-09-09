# Typsastra DOCX IR

A source-aware intermediate representation of paginated DOCX layout.

Typsastra DOCX IR maps editable WordprocessingML elements to pages, paragraphs, fitted lines, tables, cells, images, headers, footers, and notes without generating or rereading a PDF. It is designed for AI document editing, layout regression testing, template validation, and renderer interoperability.

> Editable DOCX structure, resolved into inspectable page geometry.

## Status

This repository is an early v0.1 foundation. The Rust types establish the renderer-independent format boundary. The dxpdf adapter is the next producer milestone.

The v1 contract currently models pages, source-aware paragraph segments, fitted-line UTF-8 ranges, images, overflow, and diagnostics. Table and story-region types remain planned.

## Goals

- preserve durable references to editable OOXML nodes
- expose pagination, fitted lines, bounds, overflow, tables, and anchored media
- remain independent of any particular layout or paint backend
- provide compact, versioned JSON suitable for automated tools and AI agents
- inspect layout without PDF generation, rasterization, or OCR

## CLI

Validate an IR document structurally and semantically:

```bash
cargo run -- validate examples/minimal.docx-ir.json
```

Print a compact summary:

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

## Format conventions

- coordinates and dimensions are PDF points, with 72 points per inch
- the origin is the page's top-left corner; x increases right and y increases down
- page indexes and paragraph segment indexes are zero-based
- rectangles are represented by x, y, width, and height
- line ranges are half-open UTF-8 byte ranges into their paragraph text
- source references always include the OPC part containing the editable node
- consumers must ignore unknown fields within a compatible major version
- version strings use strict, unpadded `major.minor` decimal syntax with each component in the `u16` range; this release reads supported `1.x` versions
- on the JSON wire, adding optional fields is compatible within v1; removing fields, changing meanings, or adding required enum variants requires a new major version
- unknown fields are discarded when JSON is deserialized into the typed Rust model

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

DOCX inspection will live in a separate producer adapter because accurate dxpdf layout requires native Skia. The core IR, schema, validation, summary, and future diff tooling remain Skia-free.

## Roadmap

- [x] Establish the renderer-independent Rust IR, schema, validation, and summary tooling.
- [ ] Add durable OPC-qualified source IDs with `w14:paraId` support and deterministic fallbacks.
- [ ] Build the separate dxpdf/Skia producer and standalone `inspect` command.
- [ ] Cover tables, floating objects, story regions, notes, sections, mirrored margins, and binding gutters.
- [ ] Add source-aware layout `diff` output for editing and regression workflows.
- [ ] Make output deterministic and harden processing of untrusted DOCX packages.
- [ ] Ship tested Windows, Linux, and macOS binaries that require no external PDF workflow.

See [ROADMAP.md](ROADMAP.md) for the detailed checklist, architecture boundaries, and v1 release criteria.

## License

MIT
