use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use typsastra_docx_ir::{
    AnyDocumentLayout, CompatibilityError, ProcessingLimits, read_any_document, read_document,
};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn temporary_json(name: &str, contents: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "typsastra-docx-ir-{name}-{}-{}.json",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&path, contents).unwrap();
    path
}

#[test]
fn dispatches_v1_and_v2_without_changing_the_v1_reader() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let v1 = root.join("examples/minimal.docx-ir.json");
    let v2 = root.join("examples/minimal-v2.docx-ir.json");
    assert!(matches!(
        read_any_document(&v1, &ProcessingLimits::default()).unwrap(),
        AnyDocumentLayout::V1(_)
    ));
    assert!(matches!(
        read_any_document(&v2, &ProcessingLimits::default()).unwrap(),
        AnyDocumentLayout::V2(_)
    ));
    assert!(matches!(
        read_document(&v2, &ProcessingLimits::default()).unwrap_err(),
        typsastra_docx_ir::ReadError::Compatibility(CompatibilityError::UnsupportedMajor {
            actual: 2
        })
    ));
}

#[test]
fn dispatch_accepts_future_v2_minor_fields_and_rejects_v3() {
    let example = include_str!("../examples/minimal-v2.docx-ir.json");
    let mut json: serde_json::Value = serde_json::from_str(example).unwrap();
    json["version"] = "2.1".into();
    json["future_document_field"] = serde_json::json!({"nested": true});
    json["pages"][0]["future_page_field"] = serde_json::json!(42);
    let path = temporary_json("future-v2", &serde_json::to_string(&json).unwrap());
    let document = read_any_document(&path, &ProcessingLimits::default()).unwrap();
    assert_eq!(document.version(), "2.1");
    document.validate().unwrap();
    fs::remove_file(path).unwrap();

    json["version"] = "3.0".into();
    let path = temporary_json("v3", &serde_json::to_string(&json).unwrap());
    assert!(matches!(
        read_any_document(&path, &ProcessingLimits::default()).unwrap_err(),
        typsastra_docx_ir::ReadError::Compatibility(CompatibilityError::UnsupportedMajors {
            actual: 3
        })
    ));
    fs::remove_file(path).unwrap();
}
