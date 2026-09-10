use std::{collections::HashMap, fmt, io::Read, ops::Range};

use quick_xml::{
    NsReader,
    events::{BytesStart, Event},
    name::ResolveResult,
};

use crate::{IdentityKind, LimitError, PackageBudget, ProcessingLimits, SourceRef};

pub const WML_TRANSITIONAL_NAMESPACE: &str =
    "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
pub const WML_STRICT_NAMESPACE: &str = "http://purl.oclc.org/ooxml/wordprocessingml/main";
pub const W14_NAMESPACE: &str = "http://schemas.microsoft.com/office/word/2010/wordml";
pub const STRUCTURAL_PATH_VERSION: &str = "path-v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParagraphIdentity {
    /// Zero-based physical paragraph order within the OPC part.
    pub ordinal: usize,
    /// Physical XML location retained even when a native ID is available.
    pub structural_path: String,
    pub source: SourceRef,
}

/// A paragraph identity located in the original XML byte stream.
///
/// The byte span is parser transport metadata, not part of the stable identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocatedParagraphIdentity {
    pub identity: ParagraphIdentity,
    /// Half-open raw byte range from `<` through the byte after `>` in the
    /// paragraph's opening or empty tag, including any attributes and `/>`.
    /// Offsets include a leading BOM and are valid only for the original input.
    pub start_tag_range: Range<usize>,
}

impl ParagraphIdentity {
    /// Attaches this parsed identity to data entering the processing pipeline.
    pub fn attach<T>(self, value: T) -> crate::Sourced<T> {
        crate::Sourced::new(self.source, value)
    }
}

#[derive(Debug)]
pub enum SourceIdentityError {
    InvalidPartName(String),
    InvalidXml(String),
    InvalidParaId {
        part: String,
        path: String,
        value: String,
    },
    DuplicateParaId {
        part: String,
        id: String,
        first_path: String,
        second_path: String,
    },
    Limit(LimitError),
}

impl fmt::Display for SourceIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPartName(part) => write!(formatter, "invalid OPC part name {part:?}"),
            Self::InvalidXml(message) => write!(formatter, "invalid WordprocessingML: {message}"),
            Self::InvalidParaId { part, path, value } => write!(
                formatter,
                "invalid w14:paraId {value:?} at {part}:{path} (expected eight hexadecimal digits)"
            ),
            Self::DuplicateParaId {
                part,
                id,
                first_path,
                second_path,
            } => write!(
                formatter,
                "duplicate w14:paraId {id} in {part} at {first_path} and {second_path}"
            ),
            Self::Limit(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SourceIdentityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Limit(error) => Some(error),
            _ => None,
        }
    }
}

impl From<LimitError> for SourceIdentityError {
    fn from(value: LimitError) -> Self {
        Self::Limit(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ExpandedName {
    namespace: String,
    local: String,
}

impl ExpandedName {
    fn is_paragraph(&self) -> bool {
        self.local == "p"
            && matches!(
                self.namespace.as_str(),
                WML_TRANSITIONAL_NAMESPACE | WML_STRICT_NAMESPACE
            )
    }

    fn path_step(&self, index: u64) -> String {
        format!(
            "Q{{{}}}{}[{index}]",
            escape_namespace(&self.namespace),
            self.local
        )
    }
}

#[derive(Debug)]
struct PathFrame {
    step: String,
    child_counts: HashMap<ExpandedName, u64>,
}

/// Extracts physical WordprocessingML paragraph identities from one OPC story part.
///
/// Namespace prefixes, text, formatting, comments, and attribute order do not
/// participate in generated identities. `w14:paraId` values are matched by
/// namespace URI and emitted as uppercase hexadecimal.
///
/// Each call consumes one ZIP-entry budget and charges all bytes read against
/// the per-entry and total expanded-byte limits before XML parsing begins.
pub fn extract_paragraph_identities<R: Read>(
    part: &str,
    xml: R,
    budget: &mut PackageBudget,
    limits: &ProcessingLimits,
) -> Result<Vec<ParagraphIdentity>, SourceIdentityError> {
    Ok(
        extract_paragraph_identities_impl(part, xml, budget, limits)?
            .into_iter()
            .map(|located| located.identity)
            .collect(),
    )
}

/// Extracts paragraph identities together with exact opening-tag byte ranges.
///
/// Uses the same namespace resolution, physical paths, validation, and budget
/// accounting as [`extract_paragraph_identities`]. Ranges address the raw bytes
/// read from `xml`, including any UTF-8 BOM, not decoded character positions.
/// They are parser transport metadata and must not be used as stable identities.
pub fn extract_paragraph_identities_with_spans<R: Read>(
    part: &str,
    xml: R,
    budget: &mut PackageBudget,
    limits: &ProcessingLimits,
) -> Result<Vec<LocatedParagraphIdentity>, SourceIdentityError> {
    extract_paragraph_identities_impl(part, xml, budget, limits)
}

fn extract_paragraph_identities_impl<R: Read>(
    part: &str,
    mut xml: R,
    budget: &mut PackageBudget,
    limits: &ProcessingLimits,
) -> Result<Vec<LocatedParagraphIdentity>, SourceIdentityError> {
    validate_part_name(part).map_err(|()| SourceIdentityError::InvalidPartName(part.into()))?;

    budget.begin_zip_entry(limits)?;
    let mut xml_bytes = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let read = xml
            .read(&mut chunk)
            .map_err(|error| SourceIdentityError::InvalidXml(error.to_string()))?;
        if read == 0 {
            break;
        }
        budget.add_expanded_bytes(read as u64, limits)?;
        xml_bytes.extend_from_slice(&chunk[..read]);
    }

    let mut reader = NsReader::from_reader(xml_bytes.as_slice());
    reader.config_mut().trim_text(false);
    let mut stack: Vec<PathFrame> = Vec::new();
    let mut root_counts = HashMap::new();
    let mut identities = Vec::new();
    let mut native_paths = HashMap::<String, String>::new();
    let mut root_seen = false;

    loop {
        let (resolution, event) = reader
            .read_resolved_event()
            .map_err(|error| SourceIdentityError::InvalidXml(error.to_string()))?;
        match event {
            Event::Start(element) => {
                if stack.is_empty() {
                    if root_seen {
                        return Err(SourceIdentityError::InvalidXml(
                            "multiple root elements".into(),
                        ));
                    }
                    root_seen = true;
                }
                let name = expanded_name(resolution, element.local_name().as_ref())?;
                let path = begin_element(&name, &mut stack, &mut root_counts, budget, limits)?;
                if name.is_paragraph() {
                    record_paragraph(
                        part,
                        &reader,
                        &element,
                        path,
                        &mut identities,
                        &mut native_paths,
                        start_tag_range(&reader, xml_bytes.len(), &element, false),
                    )?;
                }
                stack.push(PathFrame {
                    step: name.path_step(current_index(&name, &stack, &root_counts)),
                    child_counts: HashMap::new(),
                });
            }
            Event::Empty(element) => {
                if stack.is_empty() {
                    if root_seen {
                        return Err(SourceIdentityError::InvalidXml(
                            "multiple root elements".into(),
                        ));
                    }
                    root_seen = true;
                }
                let name = expanded_name(resolution, element.local_name().as_ref())?;
                let path = begin_element(&name, &mut stack, &mut root_counts, budget, limits)?;
                if name.is_paragraph() {
                    record_paragraph(
                        part,
                        &reader,
                        &element,
                        path,
                        &mut identities,
                        &mut native_paths,
                        start_tag_range(&reader, xml_bytes.len(), &element, true),
                    )?;
                }
            }
            Event::End(_) => {
                if stack.pop().is_none() {
                    return Err(SourceIdentityError::InvalidXml(
                        "closing element without an open element".into(),
                    ));
                }
            }
            Event::Text(text)
                if stack.is_empty()
                    && text.as_ref().iter().any(|byte| !byte.is_ascii_whitespace()) =>
            {
                return Err(SourceIdentityError::InvalidXml(
                    "non-whitespace text outside the root element".into(),
                ));
            }
            Event::CData(_) if stack.is_empty() => {
                return Err(SourceIdentityError::InvalidXml(
                    "CDATA outside the root element".into(),
                ));
            }
            Event::Eof => break,
            _ => {}
        }
    }

    if !root_seen {
        return Err(SourceIdentityError::InvalidXml(
            "missing root element".into(),
        ));
    }
    if !stack.is_empty() {
        return Err(SourceIdentityError::InvalidXml(
            "unclosed element at end of input".into(),
        ));
    }
    Ok(identities)
}

fn start_tag_range(
    reader: &NsReader<&[u8]>,
    input_len: usize,
    element: &BytesStart<'_>,
    empty: bool,
) -> Range<usize> {
    // The slice reader consumes exactly one event without read-ahead. Measuring
    // remaining raw input includes BOM bytes even if parser positions omit them.
    let end = input_len - reader.get_ref().len();
    let delimiters = if empty { 3 } else { 2 }; // `<.../>` versus `<...>`
    end - element.as_ref().len() - delimiters..end
}

fn begin_element(
    name: &ExpandedName,
    stack: &mut [PathFrame],
    root_counts: &mut HashMap<ExpandedName, u64>,
    budget: &mut PackageBudget,
    limits: &ProcessingLimits,
) -> Result<String, SourceIdentityError> {
    let depth = stack.len().saturating_add(1);
    let count = {
        let counts = stack
            .last_mut()
            .map_or(root_counts, |frame| &mut frame.child_counts);
        let count = counts.entry(name.clone()).or_default();
        *count = count.saturating_add(1);
        *count
    };
    budget.observe_xml_element(depth, limits)?;

    let mut path = String::from(STRUCTURAL_PATH_VERSION);
    path.push(':');
    for frame in stack.iter() {
        path.push('/');
        path.push_str(&frame.step);
    }
    path.push('/');
    path.push_str(&name.path_step(count));
    Ok(path)
}

fn current_index(
    name: &ExpandedName,
    stack: &[PathFrame],
    root_counts: &HashMap<ExpandedName, u64>,
) -> u64 {
    stack
        .last()
        .and_then(|frame| frame.child_counts.get(name))
        .or_else(|| root_counts.get(name))
        .copied()
        .unwrap_or(1)
}

fn record_paragraph<R: std::io::BufRead>(
    part: &str,
    reader: &NsReader<R>,
    element: &BytesStart<'_>,
    structural_path: String,
    identities: &mut Vec<LocatedParagraphIdentity>,
    native_paths: &mut HashMap<String, String>,
    start_tag_range: Range<usize>,
) -> Result<(), SourceIdentityError> {
    let mut para_id = None;
    for attribute in element.attributes() {
        let attribute =
            attribute.map_err(|error| SourceIdentityError::InvalidXml(error.to_string()))?;
        let (resolution, local) = reader.resolve_attribute(attribute.key);
        let namespace = namespace_uri(resolution)?;
        if namespace == W14_NAMESPACE && local.as_ref() == b"paraId" {
            if para_id.is_some() {
                return Err(SourceIdentityError::InvalidXml(format!(
                    "multiple w14:paraId attributes at {part}:{structural_path}"
                )));
            }
            let value = attribute
                .decode_and_unescape_value(reader.decoder())
                .map_err(|error| SourceIdentityError::InvalidXml(error.to_string()))?;
            para_id = Some(normalize_para_id(part, &structural_path, &value)?);
        }
    }

    let source = if let Some(id) = para_id {
        if let Some(first_path) = native_paths.insert(id.clone(), structural_path.clone()) {
            return Err(SourceIdentityError::DuplicateParaId {
                part: part.into(),
                id,
                first_path,
                second_path: structural_path,
            });
        }
        SourceRef {
            part: part.into(),
            id,
            identity: IdentityKind::ParaId,
        }
    } else {
        SourceRef {
            part: part.into(),
            id: structural_path.clone(),
            identity: IdentityKind::GeneratedPath,
        }
    };
    identities.push(LocatedParagraphIdentity {
        identity: ParagraphIdentity {
            ordinal: identities.len(),
            structural_path,
            source,
        },
        start_tag_range,
    });
    Ok(())
}

fn normalize_para_id(part: &str, path: &str, value: &str) -> Result<String, SourceIdentityError> {
    if value.len() != 8 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(SourceIdentityError::InvalidParaId {
            part: part.into(),
            path: path.into(),
            value: value.into(),
        });
    }
    Ok(value.to_ascii_uppercase())
}

fn expanded_name(
    resolution: ResolveResult<'_>,
    local: &[u8],
) -> Result<ExpandedName, SourceIdentityError> {
    Ok(ExpandedName {
        namespace: namespace_uri(resolution)?,
        local: std::str::from_utf8(local)
            .map_err(|error| SourceIdentityError::InvalidXml(error.to_string()))?
            .into(),
    })
}

fn namespace_uri(resolution: ResolveResult<'_>) -> Result<String, SourceIdentityError> {
    match resolution {
        ResolveResult::Bound(namespace) => std::str::from_utf8(namespace.as_ref())
            .map(str::to_owned)
            .map_err(|error| SourceIdentityError::InvalidXml(error.to_string())),
        ResolveResult::Unbound => Ok(String::new()),
        ResolveResult::Unknown(prefix) => Err(SourceIdentityError::InvalidXml(format!(
            "undeclared namespace prefix {:?}",
            String::from_utf8_lossy(prefix.as_ref())
        ))),
    }
}

fn escape_namespace(namespace: &str) -> String {
    namespace.replace('%', "%25").replace('}', "%7D")
}

pub(crate) fn validate_part_name(part: &str) -> Result<(), ()> {
    if part.is_empty()
        || part.starts_with('/')
        || part.ends_with('/')
        || part.contains(['\\', '?', '#', ':'])
        || !part.is_ascii()
        || part
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte == b' ')
        || part.split('/').any(|component| {
            component.is_empty()
                || matches!(component, "." | "..")
                || component.ends_with('.')
                || !has_canonical_percent_escapes(component)
        })
    {
        Err(())
    } else {
        Ok(())
    }
}

fn has_canonical_percent_escapes(component: &str) -> bool {
    let bytes = component.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            index += 1;
            continue;
        }
        let Some(digits) = bytes.get(index + 1..index + 3) else {
            return false;
        };
        if !digits
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(byte))
        {
            return false;
        }
        let high = hex_value(digits[0]);
        let low = hex_value(digits[1]);
        let decoded = high * 16 + low;
        if decoded.is_ascii_alphanumeric()
            || decoded.is_ascii_control()
            || matches!(decoded, b'-' | b'.' | b'_' | b'~' | b'/' | b'\\')
        {
            return false;
        }
        index += 3;
    }
    true
}

fn hex_value(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'A'..=b'F' => byte - b'A' + 10,
        _ => 0,
    }
}

pub(crate) fn is_valid_structural_path(path: &str) -> bool {
    let Some(mut remaining) = path.strip_prefix("path-v1:") else {
        return false;
    };
    let mut steps = 0usize;
    while !remaining.is_empty() {
        let Some(step) = remaining.strip_prefix("/Q{") else {
            return false;
        };
        let Some(namespace_end) = step.find('}') else {
            return false;
        };
        let namespace = &step[..namespace_end];
        if !valid_escaped_namespace(namespace) {
            return false;
        }
        let after_namespace = &step[namespace_end + 1..];
        let Some(index_start) = after_namespace.find('[') else {
            return false;
        };
        let local = &after_namespace[..index_start];
        if !is_xml_local_name(local) {
            return false;
        }
        let after_index_start = &after_namespace[index_start + 1..];
        let Some(index_end) = after_index_start.find(']') else {
            return false;
        };
        let index = &after_index_start[..index_end];
        if index.is_empty()
            || index.starts_with('0')
            || !index.bytes().all(|byte| byte.is_ascii_digit())
            || index.parse::<u64>().is_err()
        {
            return false;
        }
        remaining = &after_index_start[index_end + 1..];
        steps = steps.saturating_add(1);
    }
    steps > 0
}

pub(crate) fn is_xml_local_name(local: &str) -> bool {
    let mut characters = local.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    is_xml_name_start(first) && characters.all(is_xml_name_character)
}

fn is_xml_name_start(character: char) -> bool {
    let value = character as u32;
    character == '_'
        || character.is_ascii_alphabetic()
        || (0x00C0..=0x00D6).contains(&value)
        || (0x00D8..=0x00F6).contains(&value)
        || (0x00F8..=0x02FF).contains(&value)
        || (0x0370..=0x037D).contains(&value)
        || (0x037F..=0x1FFF).contains(&value)
        || (0x200C..=0x200D).contains(&value)
        || (0x2070..=0x218F).contains(&value)
        || (0x2C00..=0x2FEF).contains(&value)
        || (0x3001..=0xD7FF).contains(&value)
        || (0xF900..=0xFDCF).contains(&value)
        || (0xFDF0..=0xFFFD).contains(&value)
        || (0x10000..=0xEFFFF).contains(&value)
}

fn is_xml_name_character(character: char) -> bool {
    let value = character as u32;
    is_xml_name_start(character)
        || character.is_ascii_digit()
        || matches!(character, '-' | '.')
        || value == 0x00B7
        || (0x0300..=0x036F).contains(&value)
        || (0x203F..=0x2040).contains(&value)
}

fn valid_escaped_namespace(namespace: &str) -> bool {
    let mut remaining = namespace;
    while let Some(percent) = remaining.find('%') {
        let escape = remaining.get(percent..percent.saturating_add(3));
        if !matches!(escape, Some("%25" | "%7D")) {
            return false;
        }
        remaining = &remaining[percent + 3..];
    }
    !remaining.contains('}')
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn extract(xml: &str) -> Result<Vec<ParagraphIdentity>, SourceIdentityError> {
        extract_paragraph_identities(
            "word/document.xml",
            Cursor::new(xml),
            &mut PackageBudget::default(),
            &ProcessingLimits::default(),
        )
    }

    #[test]
    fn resolves_namespace_aliases_and_normalizes_para_ids() {
        let xml = format!(
            r#"<x:document xmlns:x="{WML_TRANSITIONAL_NAMESPACE}" xmlns:id="{W14_NAMESPACE}"><x:body><x:p id:paraId="abcdef12"/><x:p/></x:body></x:document>"#
        );
        let identities = extract(&xml).unwrap();
        assert_eq!(identities[0].source.id, "ABCDEF12");
        assert_eq!(identities[0].source.identity, IdentityKind::ParaId);
        assert_eq!(identities[1].source.identity, IdentityKind::GeneratedPath);
        assert!(identities[1].source.id.ends_with("}p[2]"));
    }

    #[test]
    fn rejects_invalid_and_duplicate_para_ids() {
        let invalid = format!(
            r#"<w:p xmlns:w="{WML_TRANSITIONAL_NAMESPACE}" xmlns:w14="{W14_NAMESPACE}" w14:paraId="xyz"/>"#
        );
        assert!(matches!(
            extract(&invalid),
            Err(SourceIdentityError::InvalidParaId { .. })
        ));

        let duplicate = format!(
            r#"<w:document xmlns:w="{WML_TRANSITIONAL_NAMESPACE}" xmlns:w14="{W14_NAMESPACE}"><w:p w14:paraId="1234ABCD"/><w:p w14:paraId="1234abcd"/></w:document>"#
        );
        assert!(matches!(
            extract(&duplicate),
            Err(SourceIdentityError::DuplicateParaId { .. })
        ));
    }

    #[test]
    fn enforces_expanded_part_bytes_before_xml_parsing() {
        let limits = ProcessingLimits {
            max_zip_entry_expanded_bytes: 4,
            ..ProcessingLimits::default()
        };
        let error = extract_paragraph_identities(
            "word/document.xml",
            Cursor::new("<document/>"),
            &mut PackageBudget::default(),
            &limits,
        )
        .unwrap_err();
        assert!(matches!(error, SourceIdentityError::Limit(_)));
    }

    #[test]
    fn rejects_multiple_roots_and_noncanonical_parts() {
        for xml in ["<root/><second/>", "<root>", "<root/><![CDATA[x]]>"] {
            assert!(
                matches!(extract(xml), Err(SourceIdentityError::InvalidXml(_))),
                "accepted {xml:?}"
            );
        }
        for part in [
            "/word/document.xml",
            "word/%2E%2E/evil.xml",
            "word/%ZZ.xml",
            "word/a%2Fb.xml",
            "word/a%5Cb.xml",
            "word/a%00b.xml",
            "word/document.xml.",
            "word/a:b.xml",
            " word/document.xml",
        ] {
            assert!(validate_part_name(part).is_err(), "accepted {part:?}");
        }
    }

    #[test]
    fn structural_path_validation_matches_generated_grammar() {
        assert!(is_valid_structural_path("path-v1:/Q{}document[1]/Q{}p[2]"));
        assert!(is_valid_structural_path("path-v1:/Q{}a\u{0301}[1]"));
        for path in [
            "path-v1:/Q{}not a name[1]",
            "path-v1:/Q{}p[0]",
            "path-v1:/Q{}p[01]",
            "path-v1:/Q{}p[18446744073709551616]",
            "path-v2:/Q{}p[1]",
        ] {
            assert!(!is_valid_structural_path(path), "accepted {path:?}");
        }
    }
}
