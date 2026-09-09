use std::fmt;

use crate::{FORMAT, SUPPORTED_MAJOR_VERSION};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FormatVersion {
    pub major: u16,
    pub minor: u16,
}

impl FormatVersion {
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    pub fn parse(value: &str) -> Result<Self, CompatibilityError> {
        let Some((major, minor)) = value.split_once('.') else {
            return Err(CompatibilityError::MalformedVersion(value.into()));
        };
        if major.is_empty()
            || minor.is_empty()
            || major.starts_with('0') && major != "0"
            || minor.starts_with('0') && minor != "0"
            || !major.bytes().all(|byte| byte.is_ascii_digit())
            || !minor.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(CompatibilityError::MalformedVersion(value.into()));
        }

        let major = major
            .parse()
            .map_err(|_| CompatibilityError::MalformedVersion(value.into()))?;
        let minor = minor
            .parse()
            .map_err(|_| CompatibilityError::MalformedVersion(value.into()))?;
        Ok(Self { major, minor })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompatibilityError {
    WrongFormat { actual: String },
    MalformedVersion(String),
    UnsupportedMajor { actual: u16 },
}

impl fmt::Display for CompatibilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongFormat { actual } => {
                write!(
                    formatter,
                    "unsupported format {actual:?} (expected {FORMAT:?})"
                )
            }
            Self::MalformedVersion(actual) => write!(
                formatter,
                "malformed format version {actual:?} (expected major.minor)"
            ),
            Self::UnsupportedMajor { actual } => write!(
                formatter,
                "unsupported format major version {actual} (supported: {SUPPORTED_MAJOR_VERSION})"
            ),
        }
    }
}

impl std::error::Error for CompatibilityError {}

pub fn check(format: &str, version: &str) -> Result<FormatVersion, CompatibilityError> {
    if format != FORMAT {
        return Err(CompatibilityError::WrongFormat {
            actual: format.into(),
        });
    }
    let version = FormatVersion::parse(version)?;
    if version.major != SUPPORTED_MAJOR_VERSION {
        return Err(CompatibilityError::UnsupportedMajor {
            actual: version.major,
        });
    }
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_versions_strictly() {
        assert_eq!(
            FormatVersion::parse("1.0").unwrap(),
            FormatVersion::new(1, 0)
        );
        assert_eq!(
            FormatVersion::parse("1.42").unwrap(),
            FormatVersion::new(1, 42)
        );
        assert_eq!(
            FormatVersion::parse("1.65535").unwrap(),
            FormatVersion::new(1, u16::MAX)
        );

        for malformed in [
            "1", "1.", ".0", "1.x", "01.0", "1.00", "1.0-beta", " 1.0", "1.65536",
        ] {
            assert!(
                FormatVersion::parse(malformed).is_err(),
                "accepted {malformed:?}"
            );
        }
    }
}
