use std::io::{BufReader, Cursor};

use typsastra_docx_ir::{
    IdentityKind, LocatedParagraphIdentity, PackageBudget, ProcessingLimits,
    extract_paragraph_identities, extract_paragraph_identities_with_spans,
    source_identity::{W14_NAMESPACE, WML_STRICT_NAMESPACE, WML_TRANSITIONAL_NAMESPACE},
};

const PART: &str = "word/document.xml";

fn extract(xml: &str) -> Vec<LocatedParagraphIdentity> {
    let limits = ProcessingLimits::default();
    let mut plain_budget = PackageBudget::default();
    let plain =
        extract_paragraph_identities(PART, xml.as_bytes(), &mut plain_budget, &limits).unwrap();
    let mut span_budget = PackageBudget::default();
    let located =
        extract_paragraph_identities_with_spans(PART, xml.as_bytes(), &mut span_budget, &limits)
            .unwrap();
    assert_eq!(plain_budget, span_budget);
    assert_eq!(
        plain,
        located
            .iter()
            .map(|item| item.identity.clone())
            .collect::<Vec<_>>()
    );
    located
}

fn assert_tag(xml: &str, located: &LocatedParagraphIdentity, tag: &str) {
    let start = xml.find(tag).unwrap();
    assert_eq!(located.start_tag_range, start..start + tag.len());
    assert_eq!(
        &xml.as_bytes()[located.start_tag_range.clone()],
        tag.as_bytes()
    );
}

#[test]
fn exact_tags_preserve_native_ids_and_physical_wrapper_paths() {
    for namespace in [WML_TRANSITIONAL_NAMESPACE, WML_STRICT_NAMESPACE] {
        let xml = format!(
            "\u{feff} \r\n<?probe value?>\n<root xmlns:a='{namespace}' xmlns:b='{namespace}' \
             xmlns:id='{W14_NAMESPACE}' xmlns:x='urn:ignored'>\n\
             <!-- មុន <a:p/> -->\n<x:p/><a:pPr/><a:p id:paraId='abcdef01' label='ខ្មែរ > 😀'>\
             <a:r><a:t>é😀</a:t></a:r></a:p>\n\
             <x:wrap><b:p label=\"é/>\" /><x:p/><a:p></a:p></x:wrap>\n\
             <b:p id:paraId='1234abcd'/><p xmlns='{namespace}' /></root>"
        );
        let located = extract(&xml);
        assert_eq!(located.len(), 5);
        for (item, tag) in located.iter().zip([
            "<a:p id:paraId='abcdef01' label='ខ្មែរ > 😀'>",
            "<b:p label=\"é/>\" />",
            "<a:p>",
            "<b:p id:paraId='1234abcd'/>",
            &format!("<p xmlns='{namespace}' />"),
        ]) {
            assert_tag(&xml, item, tag);
        }
        let prefix = "path-v1:/Q{}root[1]";
        let paths = [
            format!("{prefix}/Q{{{namespace}}}p[1]"),
            format!("{prefix}/Q{{urn:ignored}}wrap[1]/Q{{{namespace}}}p[1]"),
            format!("{prefix}/Q{{urn:ignored}}wrap[1]/Q{{{namespace}}}p[2]"),
            format!("{prefix}/Q{{{namespace}}}p[2]"),
            format!("{prefix}/Q{{{namespace}}}p[3]"),
        ];
        for (ordinal, (item, path)) in located.iter().zip(paths).enumerate() {
            assert_eq!(item.identity.ordinal, ordinal);
            assert_eq!(item.identity.structural_path, path);
            assert_eq!(item.identity.source.part, PART);
            if matches!(ordinal, 1 | 2 | 4) {
                assert_eq!(item.identity.source.identity, IdentityKind::GeneratedPath);
                assert_eq!(item.identity.source.id, path);
            } else {
                assert_eq!(item.identity.source.identity, IdentityKind::ParaId);
            }
        }
        assert_eq!(located[0].identity.source.id, "ABCDEF01");
        assert_eq!(located[3].identity.source.id, "1234ABCD");
    }
}

#[test]
fn root_tag_offsets_include_bom_but_exclude_leading_whitespace() {
    for prefix in ["", "\u{feff}", " \r\n\t", "\u{feff} \r\n\t"] {
        for namespace in [WML_TRANSITIONAL_NAMESPACE, WML_STRICT_NAMESPACE] {
            for suffix in ["/>", "></p>"] {
                let xml = format!("{prefix}<p xmlns='{namespace}'{suffix}");
                let located = extract(&xml);
                assert_eq!(located.len(), 1);
                let tag_end = xml.find('>').unwrap() + 1;
                assert_eq!(located[0].start_tag_range, prefix.len()..tag_end);
                assert_tag(&xml, &located[0], &xml[prefix.len()..tag_end]);
            }
        }
    }
}

#[test]
fn empty_and_start_tags_have_identical_identities() {
    for namespace in [WML_TRANSITIONAL_NAMESPACE, WML_STRICT_NAMESPACE] {
        for attribute in ["", " id:paraId='ab12cd34'"] {
            let tag = format!("<a:p xmlns:a='{namespace}' xmlns:id='{W14_NAMESPACE}'{attribute}");
            let empty_xml = format!("{tag} />");
            let start_xml = format!("{tag} >text<!-- comment --></a:p>");
            let empty = extract(&empty_xml);
            let start = extract(&start_xml);
            assert_eq!(empty[0].identity, start[0].identity);
            assert_tag(&empty_xml, &empty[0], &empty_xml);
            assert_tag(&start_xml, &start[0], &format!("{tag} >"));
        }
    }
}

#[test]
fn buffered_readers_and_multibyte_chunk_boundaries_do_not_shift_spans() {
    let xml = format!(
        "\u{feff}\n<root xmlns:w='{WML_TRANSITIONAL_NAMESPACE}'><!-- {} -->\
         <w:p label='😀 >' /><w:p>ខ្មែរ</w:p></root>",
        "é😀ខ្មែរ".repeat(2000)
    );
    let expected = extract(&xml);
    assert_tag(&xml, &expected[0], "<w:p label='😀 >' />");
    assert_tag(&xml, &expected[1], "<w:p>");
    for capacity in [1, 2, 3, 7, 8192, xml.len() * 2] {
        let reader = BufReader::with_capacity(capacity, Cursor::new(xml.as_bytes()));
        let actual = extract_paragraph_identities_with_spans(
            PART,
            reader,
            &mut PackageBudget::default(),
            &ProcessingLimits::default(),
        )
        .unwrap();
        assert_eq!(actual, expected, "buffer capacity {capacity}");
    }
}

#[test]
fn both_apis_charge_exactly_once_and_share_errors() {
    let xml = format!("\u{feff}<p xmlns='{WML_TRANSITIONAL_NAMESPACE}'/>");
    let exact = ProcessingLimits {
        max_zip_entries: 1,
        max_zip_entry_expanded_bytes: xml.len() as u64,
        max_zip_total_expanded_bytes: xml.len() as u64,
        max_xml_elements: 1,
        max_xml_nesting_depth: 1,
        ..ProcessingLimits::default()
    };
    for limits in [
        exact.clone(),
        ProcessingLimits {
            max_zip_entries: 0,
            ..exact.clone()
        },
        ProcessingLimits {
            max_zip_entry_expanded_bytes: xml.len() as u64 - 1,
            ..exact.clone()
        },
        ProcessingLimits {
            max_zip_total_expanded_bytes: xml.len() as u64 - 1,
            ..exact.clone()
        },
        ProcessingLimits {
            max_xml_elements: 0,
            ..exact.clone()
        },
        ProcessingLimits {
            max_xml_nesting_depth: 0,
            ..exact.clone()
        },
    ] {
        let mut plain_budget = PackageBudget::default();
        let mut span_budget = PackageBudget::default();
        let plain = extract_paragraph_identities(PART, xml.as_bytes(), &mut plain_budget, &limits);
        let located = extract_paragraph_identities_with_spans(
            PART,
            xml.as_bytes(),
            &mut span_budget,
            &limits,
        );
        assert_eq!(plain_budget, span_budget);
        if limits == exact {
            assert_eq!(
                plain.unwrap(),
                located
                    .unwrap()
                    .into_iter()
                    .map(|item| item.identity)
                    .collect::<Vec<_>>()
            );
        } else {
            assert_eq!(
                plain.unwrap_err().to_string(),
                located.unwrap_err().to_string()
            );
        }
    }
    for invalid_xml in [
        format!(
            "<p xmlns='{WML_TRANSITIONAL_NAMESPACE}' xmlns:id='{W14_NAMESPACE}' id:paraId='bad'/>"
        ),
        format!(
            "<root xmlns:w='{WML_TRANSITIONAL_NAMESPACE}' xmlns:id='{W14_NAMESPACE}'><w:p id:paraId='abcdef01'/><w:p id:paraId='ABCDEF01'/></root>"
        ),
        "<root><unclosed>".into(),
    ] {
        let mut plain_budget = PackageBudget::default();
        let mut span_budget = PackageBudget::default();
        let limits = ProcessingLimits::default();
        let plain =
            extract_paragraph_identities(PART, invalid_xml.as_bytes(), &mut plain_budget, &limits)
                .unwrap_err();
        let located = extract_paragraph_identities_with_spans(
            PART,
            invalid_xml.as_bytes(),
            &mut span_budget,
            &limits,
        )
        .unwrap_err();
        assert_eq!(plain.to_string(), located.to_string());
        assert_eq!(plain_budget, span_budget);
    }
}
