use typsastra_docx_ir::{
    CompatibilityError, DocumentLayout, ProcessingLimits, ReadError, read_document,
};

#[test]
fn accepts_unknown_fields_in_v1_objects() {
    let fixture = include_str!("../fixtures/compatibility/unknown-fields-v1.0.json");
    let document: DocumentLayout = serde_json::from_str(fixture).unwrap();
    document.validate().unwrap();

    let serialized = serde_json::to_string(&document).unwrap();
    assert!(!serialized.contains("future_document_field"));
    assert!(!serialized.contains("future_region_field"));
}

#[test]
fn accepts_a_future_v1_minor_version() {
    let fixture = include_str!("../fixtures/compatibility/supported-future-minor-v1.1.json");
    let document: DocumentLayout = serde_json::from_str(fixture).unwrap();
    assert_eq!(document.compatibility().unwrap().minor, 1);
    document.validate().unwrap();
}

#[test]
fn rejects_an_unknown_major_version() {
    let fixture = include_str!("../fixtures/compatibility/unsupported-major-v2.0.json");
    let document: DocumentLayout = serde_json::from_str(fixture).unwrap();
    assert!(matches!(
        document.compatibility(),
        Err(CompatibilityError::UnsupportedMajor { actual: 2 })
    ));
}

#[test]
fn bounded_reader_rejects_oversized_ir_before_parsing() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/compatibility/supported-future-minor-v1.1.json");
    let limits = ProcessingLimits {
        max_ir_json_bytes: 1,
        ..ProcessingLimits::default()
    };

    assert!(matches!(
        read_document(&path, &limits),
        Err(ReadError::Limit(_))
    ));
}
