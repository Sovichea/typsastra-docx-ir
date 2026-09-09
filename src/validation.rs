use std::{collections::HashMap, fmt};

use crate::{DocumentLayout, LayoutEnvironment, ParagraphRegion, Rect, Region, SourceRef};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationErrors(pub Vec<ValidationError>);

impl fmt::Display for ValidationErrors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, error) in self.0.iter().enumerate() {
            if index > 0 {
                writeln!(formatter)?;
            }
            write!(formatter, "{}: {}", error.path, error.message)?;
        }
        Ok(())
    }
}

impl std::error::Error for ValidationErrors {}

pub fn validate(document: &DocumentLayout) -> Result<(), ValidationErrors> {
    let mut errors = Vec::new();

    if let Err(compatibility_error) = document.compatibility() {
        errors.push(error("$", compatibility_error.to_string()));
    }
    if document.generator.name.trim().is_empty() {
        errors.push(error("generator.name", "must not be empty"));
    }
    if document.generator.version.trim().is_empty() {
        errors.push(error("generator.version", "must not be empty"));
    }
    if let Some(environment) = &document.layout_environment {
        validate_layout_environment(&mut errors, environment);
    }
    if document.source.path.trim().is_empty() {
        errors.push(error("source.path", "must not be empty"));
    }

    for (page_position, page) in document.pages.iter().enumerate() {
        let page_path = format!("pages[{page_position}]");
        if page.index != page_position {
            errors.push(error(
                format!("{page_path}.index"),
                format!("must be {page_position}, got {}", page.index),
            ));
        }
        validate_positive(
            &mut errors,
            format!("{page_path}.size_pt.width"),
            page.size_pt.width,
        );
        validate_positive(
            &mut errors,
            format!("{page_path}.size_pt.height"),
            page.size_pt.height,
        );
        if let Some(rect) = page.content_bbox_pt {
            validate_rect(&mut errors, format!("{page_path}.content_bbox_pt"), rect);
        }

        for (region_position, region) in page.regions.iter().enumerate() {
            let region_path = format!("{page_path}.regions[{region_position}]");
            match region {
                Region::Paragraph(paragraph) => {
                    validate_paragraph(&mut errors, &region_path, paragraph);
                }
                Region::Image(image) => {
                    validate_source(&mut errors, &region_path, &image.source);
                    validate_rect(&mut errors, format!("{region_path}.bbox_pt"), image.bbox_pt);
                }
            }
        }
    }

    validate_paragraph_sequences(&mut errors, document);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(ValidationErrors(errors))
    }
}

fn validate_layout_environment(errors: &mut Vec<ValidationError>, environment: &LayoutEnvironment) {
    validate_nonempty(
        errors,
        "layout_environment.engine.name",
        &environment.engine.name,
    );
    validate_nonempty(
        errors,
        "layout_environment.engine.version",
        &environment.engine.version,
    );
    validate_nonempty(
        errors,
        "layout_environment.platform.os",
        &environment.platform.os,
    );
    validate_nonempty(
        errors,
        "layout_environment.platform.architecture",
        &environment.platform.architecture,
    );
    validate_optional_nonempty(
        errors,
        "layout_environment.platform.version",
        environment.platform.version.as_deref(),
    );

    for (index, font) in environment.fonts.iter().enumerate() {
        let path = format!("layout_environment.fonts[{index}]");
        validate_nonempty(errors, format!("{path}.family"), &font.family);
        validate_optional_nonempty(
            errors,
            format!("{path}.postscript_name"),
            font.postscript_name.as_deref(),
        );
        validate_optional_nonempty(errors, format!("{path}.version"), font.version.as_deref());
        if let Some(hash) = &font.file_sha256
            && (hash.len() != 64
                || !hash
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
        {
            errors.push(error(
                format!("{path}.file_sha256"),
                "must be 64 lowercase hexadecimal characters",
            ));
        }
    }

    for (index, substitution) in environment.font_substitutions.iter().enumerate() {
        let path = format!("layout_environment.font_substitutions[{index}]");
        validate_nonempty(
            errors,
            format!("{path}.requested_family"),
            &substitution.requested_family,
        );
        validate_nonempty(
            errors,
            format!("{path}.resolved_family"),
            &substitution.resolved_family,
        );
        validate_optional_nonempty(
            errors,
            format!("{path}.resolved_postscript_name"),
            substitution.resolved_postscript_name.as_deref(),
        );
    }
}

fn validate_nonempty(errors: &mut Vec<ValidationError>, path: impl Into<String>, value: &str) {
    if value.trim().is_empty() {
        errors.push(error(path, "must not be empty"));
    }
}

fn validate_optional_nonempty(
    errors: &mut Vec<ValidationError>,
    path: impl Into<String>,
    value: Option<&str>,
) {
    if value.is_some_and(|value| value.trim().is_empty()) {
        errors.push(error(path, "must not be empty when present"));
    }
}

fn validate_paragraph(errors: &mut Vec<ValidationError>, path: &str, paragraph: &ParagraphRegion) {
    validate_source(errors, path, &paragraph.source);
    if paragraph.segment_count == 0 {
        errors.push(error(
            format!("{path}.segment_count"),
            "must be greater than zero",
        ));
    } else if paragraph.segment >= paragraph.segment_count {
        errors.push(error(
            format!("{path}.segment"),
            format!(
                "must be less than segment_count {}, got {}",
                paragraph.segment_count, paragraph.segment
            ),
        ));
    }
    validate_rect(
        errors,
        format!("{path}.frame_bbox_pt"),
        paragraph.frame_bbox_pt,
    );
    if let Some(rect) = paragraph.content_bbox_pt {
        validate_rect(errors, format!("{path}.content_bbox_pt"), rect);
    }
    if let Some(overflow) = paragraph.overflow
        && (!overflow.past_content_area_pt.is_finite() || overflow.past_content_area_pt < 0.0)
    {
        errors.push(error(
            format!("{path}.overflow.past_content_area_pt"),
            "must be finite and non-negative",
        ));
    }

    let mut previous_end = 0;
    for (line_position, line) in paragraph.lines.iter().enumerate() {
        let line_path = format!("{path}.lines[{line_position}]");
        let [start, end] = line.range_utf8;
        if start > end || end > paragraph.text.len() {
            errors.push(error(
                format!("{line_path}.range_utf8"),
                format!(
                    "must be an ordered range within text length {}, got [{start}, {end}]",
                    paragraph.text.len()
                ),
            ));
        } else {
            if !paragraph.text.is_char_boundary(start) || !paragraph.text.is_char_boundary(end) {
                errors.push(error(
                    format!("{line_path}.range_utf8"),
                    "must fall on UTF-8 code point boundaries",
                ));
            }
            if start < previous_end {
                errors.push(error(
                    format!("{line_path}.range_utf8"),
                    "must not overlap the previous line range",
                ));
            }
            previous_end = end;
        }
        validate_rect(errors, format!("{line_path}.bbox_pt"), line.bbox_pt);
    }
}

fn validate_paragraph_sequences(errors: &mut Vec<ValidationError>, document: &DocumentLayout) {
    struct ActiveSequence<'a> {
        start_path: String,
        text: &'a str,
        segment_count: usize,
        next_segment: usize,
        previous_end: usize,
        previous_page: usize,
    }

    let mut active = HashMap::<&SourceRef, ActiveSequence<'_>>::new();
    for (page_position, page) in document.pages.iter().enumerate() {
        for (region_position, region) in page.regions.iter().enumerate() {
            let Region::Paragraph(paragraph) = region else {
                continue;
            };
            if paragraph.segment_count == 0 || paragraph.segment >= paragraph.segment_count {
                continue;
            }
            let path = format!("pages[{page_position}].regions[{region_position}]");
            if paragraph.segment == 0 {
                if let Some(sequence) = active.remove(&paragraph.source) {
                    errors.push(error(
                        format!("{path}.segment"),
                        format!(
                            "starts a new occurrence before the sequence at {} reached segment_count {}",
                            sequence.start_path, sequence.segment_count
                        ),
                    ));
                }
                if paragraph.segment_count > 1 {
                    active.insert(
                        &paragraph.source,
                        ActiveSequence {
                            start_path: path,
                            text: &paragraph.text,
                            segment_count: paragraph.segment_count,
                            next_segment: 1,
                            previous_end: paragraph
                                .lines
                                .last()
                                .map_or(0, |line| line.range_utf8[1]),
                            previous_page: page_position,
                        },
                    );
                }
                continue;
            }

            let Some(sequence) = active.get_mut(&paragraph.source) else {
                errors.push(error(
                    format!("{path}.segment"),
                    "must follow segment zero for the same source occurrence",
                ));
                continue;
            };
            if page_position <= sequence.previous_page {
                errors.push(error(
                    format!("{path}.segment"),
                    "continuation segments must appear on later physical pages",
                ));
            }
            if paragraph.segment != sequence.next_segment {
                errors.push(error(
                    format!("{path}.segment"),
                    format!(
                        "must be {}, got {}",
                        sequence.next_segment, paragraph.segment
                    ),
                ));
            }
            if paragraph.segment_count != sequence.segment_count {
                errors.push(error(
                    format!("{path}.segment_count"),
                    format!(
                        "must match first segment count {}, got {}",
                        sequence.segment_count, paragraph.segment_count
                    ),
                ));
            }
            if paragraph.text != sequence.text {
                errors.push(error(
                    format!("{path}.text"),
                    "must match the logical text of the first segment",
                ));
            }
            if let Some(first_line) = paragraph.lines.first()
                && first_line.range_utf8[0] < sequence.previous_end
            {
                errors.push(error(
                    format!("{path}.lines[0].range_utf8"),
                    "must not overlap a previous page segment",
                ));
            }
            if let Some(last_line) = paragraph.lines.last() {
                sequence.previous_end = last_line.range_utf8[1];
            }
            sequence.next_segment = paragraph.segment.saturating_add(1);
            sequence.previous_page = page_position;
            if paragraph.segment.saturating_add(1) == sequence.segment_count {
                active.remove(&paragraph.source);
            }
        }
    }

    for sequence in active.into_values() {
        errors.push(error(
            format!("{}.segment_count", sequence.start_path),
            format!(
                "sequence ended before all {} segments were present",
                sequence.segment_count
            ),
        ));
    }
}

fn validate_source(errors: &mut Vec<ValidationError>, path: &str, source: &SourceRef) {
    let part_path = format!("{path}.source.part");
    if crate::source_identity::validate_part_name(&source.part).is_err() {
        errors.push(error(
            part_path,
            "must be a canonical package-relative OPC part name",
        ));
    }

    let id_path = format!("{path}.source.id");
    if source.id.trim().is_empty() {
        errors.push(error(id_path, "must not be empty"));
        return;
    }
    match source.identity {
        crate::IdentityKind::ParaId => {
            if source.id.len() != 8
                || !source
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte))
            {
                errors.push(error(
                    id_path,
                    "must contain exactly eight uppercase hexadecimal digits",
                ));
            }
        }
        crate::IdentityKind::GeneratedPath => {
            if !crate::source_identity::is_valid_structural_path(&source.id) {
                errors.push(error(id_path, "must be a versioned structural path"));
            }
        }
        crate::IdentityKind::RelationshipId => {
            if !crate::source_identity::is_xml_local_name(&source.id) {
                errors.push(error(id_path, "must be an XML NCName relationship ID"));
            }
        }
    }
}

fn validate_rect(errors: &mut Vec<ValidationError>, path: String, rect: Rect) {
    for (field, value) in [
        ("x", rect.x),
        ("y", rect.y),
        ("width", rect.width),
        ("height", rect.height),
    ] {
        if !value.is_finite() {
            errors.push(error(format!("{path}.{field}"), "must be finite"));
        } else if matches!(field, "width" | "height") && value < 0.0 {
            errors.push(error(format!("{path}.{field}"), "must be non-negative"));
        }
    }
}

fn validate_positive(errors: &mut Vec<ValidationError>, path: String, value: f32) {
    if !value.is_finite() || value <= 0.0 {
        errors.push(error(path, "must be finite and greater than zero"));
    }
}

fn error(path: impl Into<String>, message: impl Into<String>) -> ValidationError {
    ValidationError {
        path: path.into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        DocumentLayout, FontInfo, GeneratorInfo, IdentityKind, LayoutEngineInfo, LayoutEnvironment,
        LineLayout, PageLayout, ParagraphRegion, PlatformInfo, Size, SourceInfo, SourceRef,
    };

    use super::*;

    fn document_with(paragraph: ParagraphRegion) -> DocumentLayout {
        DocumentLayout::new(
            GeneratorInfo {
                name: "test".into(),
                version: "1".into(),
            },
            SourceInfo {
                path: "test.docx".into(),
                byte_length: 1,
            },
            vec![PageLayout {
                index: 0,
                number: Some(1),
                size_pt: Size {
                    width: 612.0,
                    height: 792.0,
                },
                content_bbox_pt: None,
                regions: vec![Region::Paragraph(paragraph)],
            }],
        )
    }

    fn document_with_segments(paragraphs: Vec<ParagraphRegion>) -> DocumentLayout {
        let pages = paragraphs
            .into_iter()
            .enumerate()
            .map(|(index, paragraph)| PageLayout {
                index,
                number: Some(index as u32 + 1),
                size_pt: Size {
                    width: 612.0,
                    height: 792.0,
                },
                content_bbox_pt: None,
                regions: vec![Region::Paragraph(paragraph)],
            })
            .collect();
        DocumentLayout::new(
            GeneratorInfo {
                name: "test".into(),
                version: "1".into(),
            },
            SourceInfo {
                path: "test.docx".into(),
                byte_length: 1,
            },
            pages,
        )
    }

    fn paragraph(text: &str, range: [usize; 2]) -> ParagraphRegion {
        ParagraphRegion {
            source: SourceRef {
                part: "word/document.xml".into(),
                id: "A1B2C3D4".into(),
                identity: IdentityKind::ParaId,
            },
            segment: 0,
            segment_count: 1,
            text: text.into(),
            frame_bbox_pt: Rect {
                x: 72.0,
                y: 72.0,
                width: 468.0,
                height: 14.0,
            },
            content_bbox_pt: None,
            lines: vec![LineLayout {
                range_utf8: range,
                bbox_pt: Rect {
                    x: 72.0,
                    y: 72.0,
                    width: 40.0,
                    height: 14.0,
                },
            }],
            overflow: None,
        }
    }

    #[test]
    fn accepts_valid_utf8_line_range() {
        let text = "Khmer ខ្មែរ";
        document_with(paragraph(text, [0, text.len()]))
            .validate()
            .unwrap();
    }

    #[test]
    fn rejects_range_inside_utf8_code_point() {
        let errors = document_with(paragraph("ខ", [0, 1]))
            .validate()
            .unwrap_err();
        assert!(errors.to_string().contains("UTF-8 code point boundaries"));
    }

    #[test]
    fn rejects_negative_geometry() {
        let mut paragraph = paragraph("text", [0, 4]);
        paragraph.frame_bbox_pt.width = -1.0;
        let errors = document_with(paragraph).validate().unwrap_err();
        assert!(errors.to_string().contains("width: must be non-negative"));
    }

    #[test]
    fn accepts_complete_cross_page_segment_sequence() {
        let mut first = paragraph("abcdef", [0, 3]);
        first.segment_count = 2;
        let mut second = paragraph("abcdef", [3, 6]);
        second.segment = 1;
        second.segment_count = 2;
        document_with_segments(vec![first, second])
            .validate()
            .unwrap();
    }

    #[test]
    fn rejects_incomplete_or_inconsistent_segment_sequences() {
        let mut first = paragraph("abcdef", [0, 4]);
        first.segment_count = 2;
        let mut second = paragraph("changed", [3, 7]);
        second.segment = 1;
        second.segment_count = 2;
        let errors = document_with_segments(vec![first, second])
            .validate()
            .unwrap_err()
            .to_string();
        assert!(errors.contains("must match the logical text"));
        assert!(errors.contains("must not overlap a previous page segment"));

        let mut incomplete = paragraph("abcdef", [0, 3]);
        incomplete.segment_count = 2;
        let errors = document_with_segments(vec![incomplete])
            .validate()
            .unwrap_err()
            .to_string();
        assert!(errors.contains("sequence ended before all 2 segments"));
    }

    #[test]
    fn validates_identity_kind_syntax_and_opc_qualification() {
        let mut invalid = paragraph("text", [0, 4]);
        invalid.source.part = "/word/document.xml".into();
        invalid.source.id = "not-hex".into();
        let errors = document_with(invalid).validate().unwrap_err().to_string();
        assert!(errors.contains("package-relative OPC part name"));
        assert!(errors.contains("exactly eight uppercase hexadecimal digits"));
    }

    #[test]
    fn rejects_same_page_continuations_and_noncanonical_native_ids() {
        let mut first = paragraph("abcdef", [0, 3]);
        first.segment_count = 2;
        let mut second = paragraph("abcdef", [3, 6]);
        second.segment = 1;
        second.segment_count = 2;
        second.source.id = "a1b2c3d4".into();
        first.source.id = "a1b2c3d4".into();
        let mut document = document_with(first);
        document.pages[0].regions.push(Region::Paragraph(second));

        let errors = document.validate().unwrap_err().to_string();
        assert!(errors.contains("later physical pages"));
        assert!(errors.contains("uppercase hexadecimal digits"));
    }

    #[test]
    fn rejects_invalid_relationship_ids() {
        let mut image_source = paragraph("text", [0, 4]);
        image_source.source.identity = IdentityKind::RelationshipId;
        image_source.source.id = "not an id".into();
        let errors = document_with(image_source)
            .validate()
            .unwrap_err()
            .to_string();
        assert!(errors.contains("XML NCName relationship ID"));
    }

    #[test]
    fn validates_layout_environment_metadata() {
        let mut document = document_with(paragraph("text", [0, 4]));
        document.layout_environment = Some(LayoutEnvironment {
            engine: LayoutEngineInfo {
                name: "engine".into(),
                version: "1".into(),
            },
            platform: PlatformInfo {
                os: "windows".into(),
                architecture: "x86_64".into(),
                version: None,
            },
            fonts: vec![FontInfo {
                family: "Aptos".into(),
                postscript_name: None,
                version: None,
                file_sha256: Some("ABC".into()),
                face_index: None,
            }],
            font_substitutions: Vec::new(),
        });

        let errors = document.validate().unwrap_err();
        assert!(
            errors
                .to_string()
                .contains("layout_environment.fonts[0].file_sha256")
        );
    }
}
