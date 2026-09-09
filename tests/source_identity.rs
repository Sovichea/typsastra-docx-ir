use std::{fs::File, io::BufReader, path::PathBuf};

use typsastra_docx_ir::{
    IdentityKind, PackageBudget, PaginatedParagraph, ParagraphIdentity, ParagraphSegmentDraft,
    ProcessingLimits, Rect, extract_paragraph_identities, finish_paginated_paragraph,
};

const WML: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

fn fixture(variant: &str, part: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/identity")
        .join(variant)
        .join(part)
}

fn extract(variant: &str, part: &str) -> Vec<ParagraphIdentity> {
    let file = File::open(fixture(variant, part)).unwrap();
    extract_paragraph_identities(
        part,
        BufReader::new(file),
        &mut PackageBudget::default(),
        &ProcessingLimits::default(),
    )
    .unwrap()
}

#[test]
fn identities_survive_text_style_and_namespace_prefix_edits() {
    for part in ["word/document.xml", "word/footnotes.xml"] {
        let baseline = extract("baseline", part);
        let edited = extract("text-style-edited", part);
        let baseline_sources: Vec<_> = baseline.iter().map(|item| &item.source).collect();
        let edited_sources: Vec<_> = edited.iter().map(|item| &item.source).collect();
        assert_eq!(baseline_sources, edited_sources);
        assert_eq!(baseline[0].source.identity, IdentityKind::ParaId);
        assert_eq!(baseline[1].source.identity, IdentityKind::GeneratedPath);
    }
}

#[test]
fn generated_paths_are_exact_deterministic_physical_locations() {
    let document = extract("baseline", "word/document.xml");
    assert_eq!(
        document[1].source.id,
        format!("path-v1:/Q{{{WML}}}document[1]/Q{{{WML}}}body[1]/Q{{{WML}}}p[2]")
    );

    let footnotes = extract("baseline", "word/footnotes.xml");
    assert_eq!(
        footnotes[1].source.id,
        format!("path-v1:/Q{{{WML}}}footnotes[1]/Q{{{WML}}}footnote[1]/Q{{{WML}}}p[2]")
    );
}

#[test]
fn equal_native_ids_in_different_parts_are_distinct() {
    let document = extract("baseline", "word/document.xml");
    let footnotes = extract("baseline", "word/footnotes.xml");
    assert_eq!(document[0].source.id, footnotes[0].source.id);
    assert_ne!(document[0].source, footnotes[0].source);
    assert_eq!(document[0].source.part, "word/document.xml");
    assert_eq!(footnotes[0].source.part, "word/footnotes.xml");
}

#[test]
fn extracted_provenance_survives_resolution_pagination_and_ir_conversion() {
    let identity = extract("baseline", "word/document.xml").remove(0);
    let expected_source = identity.source.clone();
    let parsed = identity.attach("parsed paragraph");
    let resolved = parsed.map(|_| "resolved style");
    let paginated = resolved.map(|_| PaginatedParagraph {
        text: "Native identity".into(),
        segments: vec![
            ParagraphSegmentDraft {
                frame_bbox_pt: Rect::default(),
                content_bbox_pt: None,
                lines: Vec::new(),
                overflow: None,
            },
            ParagraphSegmentDraft {
                frame_bbox_pt: Rect::default(),
                content_bbox_pt: None,
                lines: Vec::new(),
                overflow: None,
            },
        ],
    });

    let regions = finish_paginated_paragraph(paginated).unwrap();
    assert_eq!(regions[0].source, expected_source);
    assert_eq!(regions[1].source, expected_source);
    assert_eq!((regions[0].segment, regions[0].segment_count), (0, 2));
    assert_eq!((regions[1].segment, regions[1].segment_count), (1, 2));
}
