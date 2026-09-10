use crate::{IrStats, LimitError, ProcessingLimits};

use super::model::*;

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
            add_string(&mut stats, value);
        }
        if let Some(environment) = &self.layout_environment {
            for value in [
                &environment.engine.name,
                &environment.engine.version,
                &environment.platform.os,
                &environment.platform.architecture,
            ] {
                add_string(&mut stats, value);
            }
            if let Some(value) = &environment.platform.version {
                add_string(&mut stats, value);
            }
            stats.metadata_records = environment
                .fonts
                .len()
                .saturating_add(environment.font_substitutions.len())
                as u64;
            for font in &environment.fonts {
                add_string(&mut stats, &font.family);
                for value in [&font.postscript_name, &font.version, &font.file_sha256]
                    .into_iter()
                    .flatten()
                {
                    add_string(&mut stats, value);
                }
            }
            for substitution in &environment.font_substitutions {
                add_string(&mut stats, &substitution.requested_family);
                add_string(&mut stats, &substitution.resolved_family);
                if let Some(value) = &substitution.resolved_postscript_name {
                    add_string(&mut stats, value);
                }
            }
        }
        for section in &self.sections {
            stats.elements = stats.elements.saturating_add(1);
            add_string(&mut stats, &section.id.0);
            add_source(&mut stats, &section.source);
        }
        for diagnostic in &self.diagnostics {
            add_string(&mut stats, &diagnostic.code);
            add_string(&mut stats, &diagnostic.message);
            if let Some(source) = &diagnostic.source {
                add_source(&mut stats, source);
            }
            if let Some(region_id) = &diagnostic.region_id {
                add_string(&mut stats, &region_id.0);
            }
        }
        for page in &self.pages {
            for geometry in &page.section_geometries {
                add_string(&mut stats, &geometry.section_id.0);
                stats.elements = stats
                    .elements
                    .saturating_add(1)
                    .saturating_add(geometry.columns.len() as u64);
            }
            stats.regions = stats.regions.saturating_add(page.regions.len() as u64);
            for region in &page.regions {
                stats.elements = stats.elements.saturating_add(1);
                add_common(&mut stats, region.common());
                add_source(&mut stats, region.source());
                match region {
                    Region::Story(story) => {
                        for id in &story.linked_from {
                            add_string(&mut stats, &id.0);
                        }
                    }
                    Region::Paragraph(paragraph) => {
                        stats.lines = stats.lines.saturating_add(paragraph.lines.len() as u64);
                        stats.text_bytes =
                            stats.text_bytes.saturating_add(paragraph.text.len() as u64);
                        add_string(&mut stats, &paragraph.text);
                        stats.elements = stats
                            .elements
                            .saturating_add(paragraph.lines.len() as u64)
                            .saturating_add(paragraph.runs.len() as u64)
                            .saturating_add(paragraph.content.len() as u64);
                        for run in &paragraph.runs {
                            if let Some(source) = &run.source {
                                add_source(&mut stats, source);
                            }
                            add_string(&mut stats, &run.typography.font.family);
                            if let Some(value) = &run.typography.font.postscript_name {
                                add_string(&mut stats, value);
                            }
                        }
                        for inline in &paragraph.content {
                            if let Some(source) = &inline.source {
                                add_source(&mut stats, source);
                            }
                            match &inline.kind {
                                InlineKind::Field { instruction, .. } => {
                                    add_string(&mut stats, instruction)
                                }
                                InlineKind::NoteReference { target_story_id }
                                | InlineKind::CommentReference { target_story_id }
                                | InlineKind::Object {
                                    target_region_id: target_story_id,
                                } => add_string(&mut stats, &target_story_id.0),
                                _ => {}
                            }
                        }
                    }
                    Region::Table(table) => {
                        if let Some(floating) = &table.floating {
                            add_floating(&mut stats, floating);
                        }
                    }
                    Region::TableCell(cell) => {
                        if let Some(fill) = &cell.fill {
                            add_fill(&mut stats, fill);
                        }
                    }
                    Region::Image(image) => {
                        add_relationship(&mut stats, &image.relationship);
                        if let ObjectPlacement::Floating { placement } = &image.placement {
                            add_floating(&mut stats, placement);
                        }
                    }
                    Region::Shape(shape) => {
                        stats.elements = stats
                            .elements
                            .saturating_add(shape.path.commands.len() as u64);
                        add_fill(&mut stats, &shape.fill);
                        if let ObjectPlacement::Floating { placement } = &shape.placement {
                            add_floating(&mut stats, placement);
                        }
                    }
                    Region::TableRow(_) => {}
                }
            }
        }
        stats.elements = stats
            .elements
            .saturating_add(stats.pages)
            .saturating_add(stats.lines)
            .saturating_add(stats.diagnostics)
            .saturating_add(stats.metadata_records);
        stats
    }

    pub fn check_limits(&self, limits: &ProcessingLimits) -> Result<(), LimitError> {
        check(
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
            check(resource, observed, limit)?;
        }
        Ok(())
    }
}

fn add_common(stats: &mut IrStats, common: &RegionCommon) {
    add_string(stats, &common.id.0);
    if let Some(id) = &common.parent_id {
        add_string(stats, &id.0);
    }
    if let Some(id) = &common.owner_id {
        add_string(stats, &id.0);
    }
    if let Some(role) = &common.role {
        match &role.provenance {
            RoleProvenance::SourceDeclared {
                source,
                declaration,
            } => {
                add_source(stats, source);
                add_string(stats, declaration);
            }
            RoleProvenance::Inferred { rule } => add_string(stats, rule),
        }
    }
    if let Some(ClipGeometry::Path { path }) = &common.geometry.clip {
        stats.elements = stats.elements.saturating_add(path.commands.len() as u64);
    }
}

fn add_source(stats: &mut IrStats, source: &SourceRef) {
    add_string(stats, &source.part);
    add_string(stats, &source.id);
}

fn add_relationship(stats: &mut IrStats, relationship: &RelationshipRef) {
    add_string(stats, &relationship.source_part);
    add_string(stats, &relationship.id);
    add_string(stats, &relationship.target_part);
}

fn add_floating(stats: &mut IrStats, floating: &FloatingPlacement) {
    if let Some(path) = &floating.wrap_polygon {
        stats.elements = stats.elements.saturating_add(path.commands.len() as u64);
    }
}

fn add_fill(stats: &mut IrStats, fill: &Fill) {
    match fill {
        Fill::Image { relationship, .. } => add_relationship(stats, relationship),
        Fill::Pattern { preset, .. } => add_string(stats, preset),
        Fill::Gradient { gradient } => {
            stats.elements = stats.elements.saturating_add(gradient.stops.len() as u64)
        }
        Fill::None | Fill::Solid { .. } => {}
    }
}

fn add_string(stats: &mut IrStats, value: &str) {
    stats.string_bytes = stats.string_bytes.saturating_add(value.len() as u64);
}

fn check(resource: &'static str, observed: u64, limit: u64) -> Result<(), LimitError> {
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
