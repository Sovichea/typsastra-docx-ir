use typsastra_docx_ir::{CoverageStatus, DocumentLayout, MeasurementCoverage, to_canonical_json};

fn example_json() -> serde_json::Value {
    serde_json::from_str(include_str!("../examples/minimal.docx-ir.json")).unwrap()
}

#[test]
fn legacy_documents_do_not_acquire_coverage_claims() {
    let document: DocumentLayout = serde_json::from_value(example_json()).unwrap();
    assert_eq!(document.coverage, None);
    document.validate().unwrap();
    let json: serde_json::Value =
        serde_json::from_slice(&to_canonical_json(&document).unwrap()).unwrap();
    assert!(json.get("coverage").is_none());
}

#[test]
fn omitted_dimensions_are_unknown() {
    let mut json = example_json();
    json["coverage"] = serde_json::json!({"pagination": "complete"});
    let document: DocumentLayout = serde_json::from_value(json).unwrap();
    assert_eq!(
        document.coverage,
        Some(MeasurementCoverage {
            pagination: CoverageStatus::Complete,
            ..Default::default()
        })
    );
    document.validate().unwrap();
}

#[test]
fn coverage_round_trips_canonically() {
    let mut document: DocumentLayout = serde_json::from_value(example_json()).unwrap();
    document.coverage = Some(MeasurementCoverage {
        pagination: CoverageStatus::Complete,
        text_geometry: CoverageStatus::Partial,
        drawing_appearance: CoverageStatus::Absent,
        ..Default::default()
    });
    let bytes = to_canonical_json(&document).unwrap();
    let restored: DocumentLayout = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(restored, document);
    assert_eq!(to_canonical_json(&restored).unwrap(), bytes);
}

#[test]
fn unknown_coverage_fields_are_ignored_but_invalid_statuses_are_rejected() {
    let mut json = example_json();
    json["coverage"] = serde_json::json!({"future_measurement": "complete"});
    let document: DocumentLayout = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(document.coverage, Some(MeasurementCoverage::default()));
    json["coverage"]["pagination"] = serde_json::json!("verified");
    assert!(serde_json::from_value::<DocumentLayout>(json).is_err());
}
