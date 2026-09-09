use std::{collections::BTreeMap, fmt, io::Write};

use serde_json::Value;

use crate::{DocumentLayout, LimitError, ProcessingLimits, Rect, Region, ValidationErrors};

pub const POINT_DECIMAL_PLACES: u32 = 3;
const POINT_SCALE: f64 = 1000.0;

#[derive(Debug)]
pub enum CanonicalJsonError {
    Validation(ValidationErrors),
    Serialization(serde_json::Error),
    Limit(LimitError),
}

impl fmt::Display for CanonicalJsonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(error) => error.fmt(formatter),
            Self::Serialization(error) => error.fmt(formatter),
            Self::Limit(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CanonicalJsonError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Validation(error) => Some(error),
            Self::Serialization(error) => Some(error),
            Self::Limit(error) => Some(error),
        }
    }
}

pub fn to_canonical_json(document: &DocumentLayout) -> Result<Vec<u8>, CanonicalJsonError> {
    to_canonical_json_with_limits(document, &ProcessingLimits::default())
}

pub fn to_canonical_json_with_limits(
    document: &DocumentLayout,
    limits: &ProcessingLimits,
) -> Result<Vec<u8>, CanonicalJsonError> {
    document
        .validate()
        .map_err(CanonicalJsonError::Validation)?;
    document
        .check_limits(limits)
        .map_err(CanonicalJsonError::Limit)?;

    let mut canonical = document.clone();
    canonicalize_metadata(&mut canonical);
    canonicalize_numbers(&mut canonical);
    canonical
        .validate()
        .map_err(CanonicalJsonError::Validation)?;

    let value = serde_json::to_value(canonical).map_err(CanonicalJsonError::Serialization)?;
    let value = sort_value(value);
    let mut writer = CappedWriter::new(limits.max_generated_ir_bytes);
    if let Err(error) = serde_json::to_writer(&mut writer, &value) {
        if writer.exceeded {
            return Err(CanonicalJsonError::Limit(LimitError {
                resource: "generated IR bytes",
                observed: limits.max_generated_ir_bytes.saturating_add(1),
                limit: limits.max_generated_ir_bytes,
            }));
        }
        return Err(CanonicalJsonError::Serialization(error));
    }
    Ok(writer.bytes)
}

fn canonicalize_metadata(document: &mut DocumentLayout) {
    let Some(environment) = &mut document.layout_environment else {
        return;
    };
    environment.fonts.sort_by(|left, right| {
        (
            &left.family,
            &left.postscript_name,
            &left.version,
            &left.file_sha256,
            left.face_index,
        )
            .cmp(&(
                &right.family,
                &right.postscript_name,
                &right.version,
                &right.file_sha256,
                right.face_index,
            ))
    });
    environment.fonts.dedup();
    environment.font_substitutions.sort_by(|left, right| {
        (
            &left.requested_family,
            &left.resolved_family,
            &left.resolved_postscript_name,
        )
            .cmp(&(
                &right.requested_family,
                &right.resolved_family,
                &right.resolved_postscript_name,
            ))
    });
    environment.font_substitutions.dedup();
}

fn canonicalize_numbers(document: &mut DocumentLayout) {
    for page in &mut document.pages {
        page.size_pt.width = round_points(page.size_pt.width);
        page.size_pt.height = round_points(page.size_pt.height);
        if let Some(rect) = &mut page.content_bbox_pt {
            canonicalize_rect(rect);
        }
        for region in &mut page.regions {
            match region {
                Region::Paragraph(paragraph) => {
                    canonicalize_rect(&mut paragraph.frame_bbox_pt);
                    if let Some(rect) = &mut paragraph.content_bbox_pt {
                        canonicalize_rect(rect);
                    }
                    for line in &mut paragraph.lines {
                        canonicalize_rect(&mut line.bbox_pt);
                    }
                    if let Some(overflow) = &mut paragraph.overflow {
                        overflow.past_content_area_pt = round_points(overflow.past_content_area_pt);
                    }
                }
                Region::Image(image) => canonicalize_rect(&mut image.bbox_pt),
            }
        }
    }
}

fn canonicalize_rect(rect: &mut Rect) {
    rect.x = round_points(rect.x);
    rect.y = round_points(rect.y);
    rect.width = round_points(rect.width);
    rect.height = round_points(rect.height);
}

fn round_points(value: f32) -> f32 {
    let rounded = (f64::from(value) * POINT_SCALE).round_ties_even() / POINT_SCALE;
    let rounded = rounded as f32;
    if rounded == 0.0 { 0.0 } else { rounded }
}

fn sort_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let sorted = map
                .into_iter()
                .map(|(key, value)| (key, sort_value(value)))
                .collect::<BTreeMap<_, _>>();
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(values) => Value::Array(values.into_iter().map(sort_value).collect()),
        value => value,
    }
}

struct CappedWriter {
    bytes: Vec<u8>,
    limit: u64,
    exceeded: bool,
}

impl CappedWriter {
    fn new(limit: u64) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
            exceeded: false,
        }
    }
}

impl Write for CappedWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        if (self.bytes.len() as u64).saturating_add(buffer.len() as u64) > self.limit {
            self.exceeded = true;
            return Err(std::io::Error::other("generated IR byte limit exceeded"));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GeneratorInfo, PageLayout, Size, SourceInfo};

    #[test]
    fn canonical_json_rounds_points_and_sorts_keys() {
        let document = DocumentLayout::new(
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
                number: None,
                size_pt: Size {
                    width: 612.0004,
                    height: 791.9996,
                },
                content_bbox_pt: Some(Rect {
                    x: -0.0004,
                    y: 1.2346,
                    width: 10.0,
                    height: 20.0,
                }),
                regions: Vec::new(),
            }],
        );

        let bytes = to_canonical_json(&document).unwrap();
        let json = String::from_utf8(bytes).unwrap();
        assert!(json.starts_with("{\"format\":"));
        assert!(json.contains("\"height\":792.0"));
        assert!(json.contains("\"x\":0.0"));
        assert!(json.contains("\"y\":1.235"));
        assert!(!json.ends_with('\n'));
    }

    #[test]
    fn rejects_generated_output_over_limit() {
        let document = DocumentLayout::new(
            GeneratorInfo {
                name: "test".into(),
                version: "1".into(),
            },
            SourceInfo {
                path: "test.docx".into(),
                byte_length: 1,
            },
            Vec::new(),
        );
        let limits = ProcessingLimits {
            max_generated_ir_bytes: 1,
            ..ProcessingLimits::default()
        };
        let error = to_canonical_json_with_limits(&document, &limits).unwrap_err();
        assert!(matches!(error, CanonicalJsonError::Limit(_)));
    }
}
