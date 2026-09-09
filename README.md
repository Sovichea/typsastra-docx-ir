# Typsastra DOCX IR

A source-aware intermediate representation of paginated DOCX layout.

Typsastra DOCX IR maps editable WordprocessingML elements to pages, paragraphs, fitted lines, tables, cells, images, headers, footers, and notes without generating or rereading a PDF. It is designed for AI document editing, layout regression testing, template validation, and renderer interoperability.

> Editable DOCX structure, resolved into inspectable page geometry.

## Status

This repository is an early v0.1 foundation. The Rust types establish the renderer-independent format boundary. The dxpdf adapter and formal JSON Schema are the next milestones.

## Goals

- preserve durable references to editable OOXML nodes
- expose pagination, fitted lines, bounds, overflow, tables, and anchored media
- remain independent of any particular layout or paint backend
- provide compact, versioned JSON suitable for automated tools and AI agents
- inspect layout without PDF generation, rasterization, or OCR

## CLI

Summarize an existing IR document:

```bash
cargo run -- summary document.docx-ir.json
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
```

## Planned commands

```text
typsastra-docx-ir inspect book.docx
typsastra-docx-ir diff before.docx-ir.json after.docx-ir.json
typsastra-docx-ir validate book.docx-ir.json
typsastra-docx-ir schema
```

## License

MIT
