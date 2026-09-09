use std::{fmt, fs::File, io::Read, path::Path};

use serde::Deserialize;

use crate::{CompatibilityError, DocumentLayout, Region};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessingLimits {
    /// Maximum compressed source DOCX size.
    pub max_document_bytes: u64,
    pub max_zip_entries: u64,
    pub max_zip_entry_expanded_bytes: u64,
    pub max_zip_total_expanded_bytes: u64,
    pub max_xml_elements: u64,
    pub max_xml_nesting_depth: usize,
    pub max_ir_json_bytes: u64,
    pub max_json_nesting_depth: usize,
    pub max_pages: u64,
    pub max_regions: u64,
    pub max_lines: u64,
    pub max_diagnostics: u64,
    pub max_text_bytes: u64,
    pub max_string_bytes: u64,
    pub max_elements: u64,
    pub max_generated_ir_bytes: u64,
}

impl Default for ProcessingLimits {
    fn default() -> Self {
        Self {
            max_document_bytes: 100 * 1024 * 1024,
            max_zip_entries: 10_000,
            max_zip_entry_expanded_bytes: 256 * 1024 * 1024,
            max_zip_total_expanded_bytes: 1024 * 1024 * 1024,
            max_xml_elements: 10_000_000,
            max_xml_nesting_depth: 128,
            max_ir_json_bytes: 256 * 1024 * 1024,
            max_json_nesting_depth: 64,
            max_pages: 10_000,
            max_regions: 1_000_000,
            max_lines: 5_000_000,
            max_diagnostics: 100_000,
            max_text_bytes: 128 * 1024 * 1024,
            max_string_bytes: 192 * 1024 * 1024,
            max_elements: 6_000_000,
            max_generated_ir_bytes: 256 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PackageBudget {
    zip_entries: u64,
    current_entry_expanded_bytes: u64,
    total_expanded_bytes: u64,
    xml_elements: u64,
}

impl PackageBudget {
    pub fn check_document_bytes(
        &self,
        observed: u64,
        limits: &ProcessingLimits,
    ) -> Result<(), LimitError> {
        check_limit("source document bytes", observed, limits.max_document_bytes)
    }

    pub fn begin_zip_entry(&mut self, limits: &ProcessingLimits) -> Result<(), LimitError> {
        self.zip_entries = self.zip_entries.saturating_add(1);
        check_limit("ZIP entries", self.zip_entries, limits.max_zip_entries)?;
        self.current_entry_expanded_bytes = 0;
        Ok(())
    }

    pub fn add_expanded_bytes(
        &mut self,
        bytes: u64,
        limits: &ProcessingLimits,
    ) -> Result<(), LimitError> {
        self.current_entry_expanded_bytes = self.current_entry_expanded_bytes.saturating_add(bytes);
        self.total_expanded_bytes = self.total_expanded_bytes.saturating_add(bytes);
        check_limit(
            "expanded ZIP entry bytes",
            self.current_entry_expanded_bytes,
            limits.max_zip_entry_expanded_bytes,
        )?;
        check_limit(
            "total expanded ZIP bytes",
            self.total_expanded_bytes,
            limits.max_zip_total_expanded_bytes,
        )
    }

    pub fn observe_xml_element(
        &mut self,
        depth: usize,
        limits: &ProcessingLimits,
    ) -> Result<(), LimitError> {
        self.xml_elements = self.xml_elements.saturating_add(1);
        check_limit("XML elements", self.xml_elements, limits.max_xml_elements)?;
        check_limit(
            "XML nesting depth",
            depth as u64,
            limits.max_xml_nesting_depth as u64,
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IrStats {
    pub pages: u64,
    pub regions: u64,
    pub lines: u64,
    pub diagnostics: u64,
    pub metadata_records: u64,
    pub text_bytes: u64,
    pub string_bytes: u64,
    pub elements: u64,
}

impl DocumentLayout {
    pub fn stats(&self) -> IrStats {
        let mut stats = IrStats {
            pages: self.pages.len() as u64,
            diagnostics: self.diagnostics.len() as u64,
            ..IrStats::default()
        };
        for value in [
            &self.format,
            &self.version,
            &self.generator.name,
            &self.generator.version,
            &self.source.path,
        ] {
            add_string_bytes(&mut stats, value);
        }
        if let Some(environment) = &self.layout_environment {
            for value in [
                &environment.engine.name,
                &environment.engine.version,
                &environment.platform.os,
                &environment.platform.architecture,
            ] {
                add_string_bytes(&mut stats, value);
            }
            if let Some(version) = &environment.platform.version {
                add_string_bytes(&mut stats, version);
            }
            stats.metadata_records = (environment.fonts.len() as u64)
                .saturating_add(environment.font_substitutions.len() as u64);
            for font in &environment.fonts {
                add_string_bytes(&mut stats, &font.family);
                for value in [&font.postscript_name, &font.version, &font.file_sha256]
                    .into_iter()
                    .flatten()
                {
                    add_string_bytes(&mut stats, value);
                }
            }
            for substitution in &environment.font_substitutions {
                add_string_bytes(&mut stats, &substitution.requested_family);
                add_string_bytes(&mut stats, &substitution.resolved_family);
                if let Some(name) = &substitution.resolved_postscript_name {
                    add_string_bytes(&mut stats, name);
                }
            }
        }
        for diagnostic in &self.diagnostics {
            add_string_bytes(&mut stats, &diagnostic.code);
            add_string_bytes(&mut stats, &diagnostic.message);
            if let Some(source_id) = &diagnostic.source_id {
                add_string_bytes(&mut stats, source_id);
            }
        }
        for page in &self.pages {
            stats.regions = stats.regions.saturating_add(page.regions.len() as u64);
            for region in &page.regions {
                let source = match region {
                    Region::Paragraph(paragraph) => {
                        stats.lines = stats.lines.saturating_add(paragraph.lines.len() as u64);
                        stats.text_bytes =
                            stats.text_bytes.saturating_add(paragraph.text.len() as u64);
                        add_string_bytes(&mut stats, &paragraph.text);
                        &paragraph.source
                    }
                    Region::Image(image) => {
                        if let Some(owner_id) = &image.owner_id {
                            add_string_bytes(&mut stats, owner_id);
                        }
                        &image.source
                    }
                };
                add_string_bytes(&mut stats, &source.part);
                add_string_bytes(&mut stats, &source.id);
            }
        }
        stats.elements = stats
            .pages
            .saturating_add(stats.regions)
            .saturating_add(stats.lines)
            .saturating_add(stats.diagnostics)
            .saturating_add(stats.metadata_records);
        stats
    }

    pub fn check_limits(&self, limits: &ProcessingLimits) -> Result<(), LimitError> {
        check_limit(
            "source document bytes",
            self.source.byte_length,
            limits.max_document_bytes,
        )?;
        let stats = self.stats();
        for (resource, observed, limit) in [
            ("pages", stats.pages, limits.max_pages),
            ("regions", stats.regions, limits.max_regions),
            ("lines", stats.lines, limits.max_lines),
            ("diagnostics", stats.diagnostics, limits.max_diagnostics),
            (
                "paragraph text bytes",
                stats.text_bytes,
                limits.max_text_bytes,
            ),
            (
                "IR string bytes",
                stats.string_bytes,
                limits.max_string_bytes,
            ),
            ("IR elements", stats.elements, limits.max_elements),
        ] {
            check_limit(resource, observed, limit)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LimitError {
    pub resource: &'static str,
    pub observed: u64,
    pub limit: u64,
}

impl fmt::Display for LimitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} exceeds limit of {} (observed {})",
            self.resource, self.limit, self.observed
        )
    }
}

impl std::error::Error for LimitError {}

fn add_string_bytes(stats: &mut IrStats, value: &str) {
    stats.string_bytes = stats.string_bytes.saturating_add(value.len() as u64);
}

fn check_limit(resource: &'static str, observed: u64, limit: u64) -> Result<(), LimitError> {
    if observed > limit {
        Err(LimitError {
            resource,
            observed,
            limit,
        })
    } else {
        Ok(())
    }
}

#[derive(Debug)]
pub enum ReadError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Compatibility(CompatibilityError),
    Limit(LimitError),
}

impl fmt::Display for ReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Json(error) => error.fmt(formatter),
            Self::Compatibility(error) => error.fmt(formatter),
            Self::Limit(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::Compatibility(error) => Some(error),
            Self::Limit(error) => Some(error),
        }
    }
}

impl From<std::io::Error> for ReadError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for ReadError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<CompatibilityError> for ReadError {
    fn from(value: CompatibilityError) -> Self {
        Self::Compatibility(value)
    }
}

impl From<LimitError> for ReadError {
    fn from(value: LimitError) -> Self {
        Self::Limit(value)
    }
}

#[derive(Deserialize)]
struct DocumentEnvelope {
    format: String,
    version: String,
}

pub fn read_document(path: &Path, limits: &ProcessingLimits) -> Result<DocumentLayout, ReadError> {
    let file = File::open(path)?;
    if file.metadata()?.len() > limits.max_ir_json_bytes {
        return Err(LimitError {
            resource: "IR JSON input bytes",
            observed: file.metadata()?.len(),
            limit: limits.max_ir_json_bytes,
        }
        .into());
    }

    let read_cap = limits.max_ir_json_bytes.saturating_add(1);
    let mut bytes = Vec::new();
    file.take(read_cap).read_to_end(&mut bytes)?;
    check_limit(
        "IR JSON input bytes",
        bytes.len() as u64,
        limits.max_ir_json_bytes,
    )?;
    check_json_depth(&bytes, limits.max_json_nesting_depth)?;

    let envelope: DocumentEnvelope = serde_json::from_slice(&bytes)?;
    crate::compatibility::check(&envelope.format, &envelope.version)?;
    let document: DocumentLayout = serde_json::from_slice(&bytes)?;
    document.check_limits(limits)?;
    Ok(document)
}

fn check_json_depth(bytes: &[u8], max_depth: usize) -> Result<(), LimitError> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for &byte in bytes {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth = depth.saturating_add(1);
                if depth > max_depth {
                    return Err(LimitError {
                        resource: "JSON nesting depth",
                        observed: depth as u64,
                        limit: max_depth as u64,
                    });
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GeneratorInfo, SourceInfo};

    #[test]
    fn counts_and_limits_ir_elements() {
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
            max_pages: 0,
            ..ProcessingLimits::default()
        };
        assert!(document.check_limits(&limits).is_ok());
        let limits = ProcessingLimits {
            max_document_bytes: 0,
            max_pages: 0,
            ..ProcessingLimits::default()
        };
        assert_eq!(
            document.check_limits(&limits).unwrap_err().resource,
            "source document bytes"
        );
    }

    #[test]
    fn depth_scanner_ignores_json_punctuation_in_strings() {
        check_json_depth(br#"{"value":"[[[\\\"{{{"}"#, 1).unwrap();
        let error = check_json_depth(b"[[[]]]", 2).unwrap_err();
        assert_eq!(error.resource, "JSON nesting depth");
        assert_eq!(error.observed, 3);
    }

    #[test]
    fn metadata_records_and_strings_participate_in_ir_limits() {
        let mut document = DocumentLayout::new(
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
        document.layout_environment = Some(crate::LayoutEnvironment {
            engine: crate::LayoutEngineInfo {
                name: "engine".into(),
                version: "1".into(),
            },
            platform: crate::PlatformInfo {
                os: "windows".into(),
                architecture: "x86_64".into(),
                version: None,
            },
            fonts: vec![crate::FontInfo {
                family: "Aptos".into(),
                postscript_name: None,
                version: None,
                file_sha256: None,
                face_index: None,
            }],
            font_substitutions: Vec::new(),
        });
        let stats = document.stats();
        assert_eq!(stats.metadata_records, 1);
        assert!(stats.string_bytes > stats.text_bytes);

        let limits = ProcessingLimits {
            max_elements: 0,
            ..ProcessingLimits::default()
        };
        assert_eq!(
            document.check_limits(&limits).unwrap_err().resource,
            "IR elements"
        );
    }

    #[test]
    fn package_budget_stops_zip_expansion_and_xml_growth() {
        let limits = ProcessingLimits {
            max_zip_entries: 1,
            max_zip_entry_expanded_bytes: 3,
            max_xml_elements: 1,
            max_xml_nesting_depth: 2,
            ..ProcessingLimits::default()
        };

        let mut budget = PackageBudget::default();
        budget.begin_zip_entry(&limits).unwrap();
        budget.add_expanded_bytes(3, &limits).unwrap();
        assert_eq!(
            budget.add_expanded_bytes(1, &limits).unwrap_err().resource,
            "expanded ZIP entry bytes"
        );
        assert_eq!(
            budget.begin_zip_entry(&limits).unwrap_err().resource,
            "ZIP entries"
        );

        let mut budget = PackageBudget::default();
        budget.observe_xml_element(2, &limits).unwrap();
        assert_eq!(
            budget.observe_xml_element(2, &limits).unwrap_err().resource,
            "XML elements"
        );
        let mut budget = PackageBudget::default();
        assert_eq!(
            budget.observe_xml_element(3, &limits).unwrap_err().resource,
            "XML nesting depth"
        );
    }
}
