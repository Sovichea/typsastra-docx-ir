use std::fmt;

use crate::{LineLayout, Overflow, ParagraphRegion, Rect, SourceRef};

/// A value whose source identity is moved unchanged through processing stages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sourced<T> {
    source: SourceRef,
    value: T,
}

impl<T> Sourced<T> {
    pub fn new(source: SourceRef, value: T) -> Self {
        Self { source, value }
    }

    pub fn source(&self) -> &SourceRef {
        &self.source
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn value_mut(&mut self) -> &mut T {
        &mut self.value
    }

    pub fn into_parts(self) -> (SourceRef, T) {
        (self.source, self.value)
    }

    pub fn map<U>(self, transform: impl FnOnce(T) -> U) -> Sourced<U> {
        Sourced {
            source: self.source,
            value: transform(self.value),
        }
    }

    pub fn try_map<U, E>(self, transform: impl FnOnce(T) -> Result<U, E>) -> Result<Sourced<U>, E> {
        Ok(Sourced {
            source: self.source,
            value: transform(self.value)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParagraphSegmentDraft {
    pub frame_bbox_pt: Rect,
    pub content_bbox_pt: Option<Rect>,
    pub lines: Vec<LineLayout>,
    pub overflow: Option<Overflow>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PaginatedParagraph {
    pub text: String,
    pub segments: Vec<ParagraphSegmentDraft>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmptyParagraphSegments;

impl fmt::Display for EmptyParagraphSegments {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a paginated paragraph must contain at least one segment")
    }
}

impl std::error::Error for EmptyParagraphSegments {}

/// Assigns zero-based segment indexes only after pagination has produced every
/// fragment. Every resulting region receives the exact same source identity.
pub fn finish_paginated_paragraph(
    paragraph: Sourced<PaginatedParagraph>,
) -> Result<Vec<ParagraphRegion>, EmptyParagraphSegments> {
    let (source, paragraph) = paragraph.into_parts();
    if paragraph.segments.is_empty() {
        return Err(EmptyParagraphSegments);
    }
    let segment_count = paragraph.segments.len();
    Ok(paragraph
        .segments
        .into_iter()
        .enumerate()
        .map(|(segment, draft)| ParagraphRegion {
            source: source.clone(),
            segment,
            segment_count,
            text: paragraph.text.clone(),
            frame_bbox_pt: draft.frame_bbox_pt,
            content_bbox_pt: draft.content_bbox_pt,
            lines: draft.lines,
            overflow: draft.overflow,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IdentityKind, Rect};

    #[test]
    fn provenance_survives_stage_mapping_and_page_splitting() {
        let source = SourceRef {
            part: "word/document.xml".into(),
            id: "1234ABCD".into(),
            identity: IdentityKind::ParaId,
        };
        let parsed = Sourced::new(source.clone(), "parsed");
        let styled = parsed.map(|_| "styled");
        let paginated = styled.map(|_| PaginatedParagraph {
            text: "one paragraph".into(),
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
        assert_eq!(regions[0].source, source);
        assert_eq!(regions[1].source, source);
        assert_eq!((regions[0].segment, regions[0].segment_count), (0, 2));
        assert_eq!((regions[1].segment, regions[1].segment_count), (1, 2));
    }
}
