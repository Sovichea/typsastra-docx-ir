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
```

## Format conventions

- coordinates and dimensions are PDF points, with 72 points per inch
- the origin is the page's top-left corner; x increases right and y increases down
- page indexes and paragraph segment indexes are zero-based
- rectangles are represented by x, y, width, and height
- line ranges are half-open UTF-8 byte ranges into their paragraph text
- source references always include the OPC part containing the editable node
- consumers must ignore unknown fields within a compatible major version

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
