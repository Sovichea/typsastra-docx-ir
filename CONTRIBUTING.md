# Contributing

Thank you for helping build Typsastra DOCX IR.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Keep the public IR renderer-independent. Schema changes must document compatibility, coordinate units, source identity behavior, and text-range semantics. Add focused tests for every new region type or serialization change.
