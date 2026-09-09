use std::fmt;

use crate::{DocumentLayout, ParagraphRegion, Rect, Region, SourceRef};

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

    if !document.is_supported() {
        errors.push(error(
            "$",
            format!(
                "unsupported format {} {} (expected {} 1.x)",
                document.format,
                document.version,
                crate::FORMAT
            ),
        ));
    }
    if document.generator.name.trim().is_empty() {
        errors.push(error("generator.name", "must not be empty"));
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

    if errors.is_empty() {
        Ok(())
    } else {
        Err(ValidationErrors(errors))
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

fn validate_source(errors: &mut Vec<ValidationError>, path: &str, source: &SourceRef) {
    if source.part.trim().is_empty() {
        errors.push(error(format!("{path}.source.part"), "must not be empty"));
    }
    if source.id.trim().is_empty() {
        errors.push(error(format!("{path}.source.id"), "must not be empty"));
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
        DocumentLayout, GeneratorInfo, IdentityKind, LineLayout, PageLayout, ParagraphRegion, Size,
        SourceInfo, SourceRef,
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
}
