//! Version-aware reading without weakening either typed contract.

use std::{fmt, fs::File, io::Read, path::Path};

use serde::Deserialize;

use crate::{DocumentLayout, LimitError, ProcessingLimits, ReadError, v2};

#[derive(Clone, Debug, PartialEq)]
pub enum AnyDocumentLayout {
    V1(DocumentLayout),
    V2(v2::DocumentLayout),
}

impl AnyDocumentLayout {
    pub fn format(&self) -> &str {
        match self {
            Self::V1(v) => &v.format,
            Self::V2(v) => &v.format,
        }
    }
    pub fn version(&self) -> &str {
        match self {
            Self::V1(v) => &v.version,
            Self::V2(v) => &v.version,
        }
    }
    pub fn validate(&self) -> Result<(), AnyValidationErrors> {
        match self {
            Self::V1(v) => v.validate().map_err(AnyValidationErrors::V1),
            Self::V2(v) => v.validate().map_err(AnyValidationErrors::V2),
        }
    }
}

#[derive(Debug)]
pub enum AnyValidationErrors {
    V1(crate::ValidationErrors),
    V2(v2::ValidationErrors),
}
impl fmt::Display for AnyValidationErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::V1(e) => e.fmt(f),
            Self::V2(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for AnyValidationErrors {}

#[derive(Deserialize)]
struct Envelope {
    format: String,
    version: String,
}

pub fn detect_document_version(
    path: &Path,
    limits: &ProcessingLimits,
) -> Result<crate::FormatVersion, ReadError> {
    let file = File::open(path)?;
    let length = file.metadata()?.len();
    if length > limits.max_ir_json_bytes {
        return Err(LimitError {
            resource: "IR JSON input bytes",
            observed: length,
            limit: limits.max_ir_json_bytes,
        }
        .into());
    }
    let mut bytes = Vec::new();
    file.take(limits.max_ir_json_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    crate::limits::check_json_depth(&bytes, limits.max_json_nesting_depth)?;
    let envelope: Envelope = serde_json::from_slice(&bytes)?;
    Ok(crate::compatibility::detect(
        &envelope.format,
        &envelope.version,
    )?)
}

pub fn read_any_document(
    path: &Path,
    limits: &ProcessingLimits,
) -> Result<AnyDocumentLayout, ReadError> {
    let file = File::open(path)?;
    let length = file.metadata()?.len();
    if length > limits.max_ir_json_bytes {
        return Err(LimitError {
            resource: "IR JSON input bytes",
            observed: length,
            limit: limits.max_ir_json_bytes,
        }
        .into());
    }
    let mut bytes = Vec::new();
    file.take(limits.max_ir_json_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limits.max_ir_json_bytes {
        return Err(LimitError {
            resource: "IR JSON input bytes",
            observed: bytes.len() as u64,
            limit: limits.max_ir_json_bytes,
        }
        .into());
    }
    crate::limits::check_json_depth(&bytes, limits.max_json_nesting_depth)
        .map_err(ReadError::from)?;
    let envelope: Envelope = serde_json::from_slice(&bytes)?;
    let version = crate::compatibility::detect(&envelope.format, &envelope.version)?;
    match version.major {
        1 => {
            let d: DocumentLayout = serde_json::from_slice(&bytes)?;
            d.check_limits(limits)?;
            Ok(AnyDocumentLayout::V1(d))
        }
        2 => {
            let d: v2::DocumentLayout = serde_json::from_slice(&bytes)?;
            d.check_limits(limits)?;
            Ok(AnyDocumentLayout::V2(d))
        }
        actual => Err(crate::CompatibilityError::UnsupportedMajors { actual }.into()),
    }
}
