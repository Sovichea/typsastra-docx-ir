use std::{collections::BTreeMap, io::Write};

use serde_json::Value;

use crate::{CanonicalJsonError, LimitError, ProcessingLimits};

use super::model::DocumentLayout;

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
    let value = serde_json::to_value(canonical).map_err(CanonicalJsonError::Serialization)?;
    let value = canonicalize_value(value);
    let canonical: DocumentLayout =
        serde_json::from_value(value.clone()).map_err(CanonicalJsonError::Serialization)?;
    canonical
        .validate()
        .map_err(CanonicalJsonError::Validation)?;

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

fn canonicalize_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let sorted = map
                .into_iter()
                .map(|(key, value)| (key, canonicalize_value(value)))
                .collect::<BTreeMap<_, _>>();
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_value).collect()),
        Value::Number(number) if !number.is_i64() && !number.is_u64() => {
            let value = number.as_f64().expect("JSON numbers are finite");
            let rounded = (value * 1000.0).round_ties_even() / 1000.0;
            let rounded = if rounded == 0.0 { 0.0 } else { rounded };
            Value::Number(
                serde_json::Number::from_f64(rounded)
                    .expect("canonicalized JSON numbers remain finite"),
            )
        }
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
