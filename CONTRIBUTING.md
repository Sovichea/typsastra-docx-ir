# Contributing

Thank you for helping build Typsastra DOCX IR.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo run -- schema --output schema/typsastra-docx-ir-v1.schema.json
git diff --exit-code -- schema/typsastra-docx-ir-v1.schema.json
```

Keep the public IR renderer-independent. Schema changes must document compatibility, coordinate units, source identity behavior, and text-range semantics. Add focused tests for every new region type or serialization change.

Source identity changes must use namespace-aware parsing, compare complete OPC-qualified references, and include fixtures for both native IDs and generated paths. Never inject bookmarks, hidden text, or marker runs to recover identity.
