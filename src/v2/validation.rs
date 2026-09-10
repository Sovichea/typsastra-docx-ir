use std::collections::{HashMap, HashSet};

use crate::{ValidationError, ValidationErrors};

use super::model::*;

pub fn validate(document: &DocumentLayout) -> Result<(), ValidationErrors> {
    let mut errors = Vec::new();

    if let Err(compatibility_error) = document.compatibility() {
        errors.push(error("$", compatibility_error.to_string()));
    }
    validate_nonempty(&mut errors, "generator.name", &document.generator.name);
    validate_nonempty(
        &mut errors,
        "generator.version",
        &document.generator.version,
    );
    validate_nonempty(&mut errors, "source.path", &document.source.path);
    if let Some(environment) = &document.layout_environment {
        validate_environment(&mut errors, environment);
    }
    validate_sections(&mut errors, document);

    let mut regions = HashMap::<&RegionId, (usize, usize, &Region)>::new();
    for (page_position, page) in document.pages.iter().enumerate() {
        validate_page(&mut errors, document, page_position, page);
        for (region_position, region) in page.regions.iter().enumerate() {
            let path = format!("pages[{page_position}].regions[{region_position}]");
            validate_region(&mut errors, &path, region);
            let id = &region.common().id;
            validate_nonempty(&mut errors, format!("{path}.id"), &id.0);
            if let Some((first_page, first_region, _)) =
                regions.insert(id, (page_position, region_position, region))
            {
                errors.push(error(
                    format!("{path}.id"),
                    format!(
                        "must be unique; first used at pages[{first_page}].regions[{first_region}]"
                    ),
                ));
            }
        }
    }

    validate_region_graph(&mut errors, &regions);
    validate_reading_order(&mut errors, &regions);
    validate_fragment_sequences(&mut errors, document);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(ValidationErrors(errors))
    }
}

fn validate_environment(errors: &mut Vec<ValidationError>, environment: &LayoutEnvironment) {
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

fn validate_sections(errors: &mut Vec<ValidationError>, document: &DocumentLayout) {
    let mut ids = HashSet::new();
    let mut previous_start = 0usize;
    for (position, section) in document.sections.iter().enumerate() {
        let path = format!("sections[{position}]");
        if section.index != position {
            errors.push(error(
                format!("{path}.index"),
                format!("must be {position}, got {}", section.index),
            ));
        }
        validate_nonempty(errors, format!("{path}.id"), &section.id.0);
        if !ids.insert(&section.id) {
            errors.push(error(format!("{path}.id"), "must be unique"));
        }
        validate_source(errors, &format!("{path}.source"), &section.source);
        let [start, end] = section.page_range;
        if start < previous_start || start >= end || end > document.pages.len() {
            errors.push(error(
                format!("{path}.page_range"),
                format!(
                    "must be a non-empty ordered range within {} pages",
                    document.pages.len()
                ),
            ));
        }
        previous_start = start;
    }
    if document.pages.is_empty() {
        if !document.sections.is_empty() {
            errors.push(error("sections", "must be empty when pages is empty"));
        }
    } else if document.sections.is_empty() {
        errors.push(error("sections", "must describe every recorded page"));
    }
}

fn validate_page(
    errors: &mut Vec<ValidationError>,
    document: &DocumentLayout,
    position: usize,
    page: &PageLayout,
) {
    let path = format!("pages[{position}]");
    if page.index != position {
        errors.push(error(
            format!("{path}.index"),
            format!("must be {position}, got {}", page.index),
        ));
    }
    let expected_sections = document
        .sections
        .iter()
        .filter(|section| position >= section.page_range[0] && position < section.page_range[1])
        .collect::<Vec<_>>();
    if page.section_geometries.len() != expected_sections.len() {
        errors.push(error(
            format!("{path}.section_geometries"),
            "must contain one entry for every section occurring on this page",
        ));
    }
    for (geometry_position, geometry) in page.section_geometries.iter().enumerate() {
        let geometry_path = format!("{path}.section_geometries[{geometry_position}]");
        match expected_sections.get(geometry_position) {
            Some(section) if section.id == geometry.section_id => {}
            Some(_) => errors.push(error(
                format!("{geometry_path}.section_id"),
                "must follow document section order for this page",
            )),
            None => errors.push(error(
                format!("{geometry_path}.section_id"),
                "does not correspond to a section on this page",
            )),
        }
        validate_insets(
            errors,
            &format!("{geometry_path}.margins_pt"),
            geometry.margins_pt,
            false,
        );
        validate_nonnegative(
            errors,
            format!("{geometry_path}.gutter_pt"),
            geometry.gutter_pt,
        );
        validate_rect(
            errors,
            &format!("{geometry_path}.usable_content_bbox_pt"),
            geometry.usable_content_bbox_pt,
        );
        for (column_position, column) in geometry.columns.iter().enumerate() {
            let column_path = format!("{geometry_path}.columns[{column_position}]");
            if column.index != column_position {
                errors.push(error(
                    format!("{column_path}.index"),
                    format!("must be {column_position}, got {}", column.index),
                ));
            }
            validate_rect(errors, &format!("{column_path}.bbox_pt"), column.bbox_pt);
        }
    }
    validate_positive(errors, format!("{path}.size_pt.width"), page.size_pt.width);
    validate_positive(
        errors,
        format!("{path}.size_pt.height"),
        page.size_pt.height,
    );
}

fn validate_region(errors: &mut Vec<ValidationError>, path: &str, region: &Region) {
    validate_source(errors, &format!("{path}.source"), region.source());
    validate_geometry(
        errors,
        &format!("{path}.geometry"),
        &region.common().geometry,
    );
    if let Some(role) = &region.common().role {
        validate_role(errors, &format!("{path}.role"), role);
    }
    match region {
        Region::Story(story) => {
            for (index, id) in story.linked_from.iter().enumerate() {
                validate_nonempty(errors, format!("{path}.linked_from[{index}]"), &id.0);
            }
        }
        Region::Paragraph(paragraph) => validate_paragraph(errors, path, paragraph),
        Region::Table(table) => {
            validate_fragment(errors, &format!("{path}.fragment"), table.fragment);
            validate_rect(
                errors,
                &format!("{path}.content_bbox_pt"),
                table.content_bbox_pt,
            );
            validate_nonnegative(
                errors,
                format!("{path}.cell_spacing_pt"),
                table.cell_spacing_pt,
            );
            validate_borders(errors, &format!("{path}.borders"), table.borders);
            if let Some(floating) = &table.floating {
                validate_floating(errors, &format!("{path}.floating"), floating);
            }
            validate_overflow(errors, path, table.overflow);
        }
        Region::TableRow(row) => {
            validate_fragment(errors, &format!("{path}.fragment"), row.fragment);
            validate_overflow(errors, path, row.overflow);
        }
        Region::TableCell(cell) => {
            validate_fragment(errors, &format!("{path}.fragment"), cell.fragment);
            if cell.grid_span == 0 {
                errors.push(error(
                    format!("{path}.grid_span"),
                    "must be greater than zero",
                ));
            }
            if cell.row_span == 0 {
                errors.push(error(
                    format!("{path}.row_span"),
                    "must be greater than zero",
                ));
            }
            validate_rect(
                errors,
                &format!("{path}.content_bbox_pt"),
                cell.content_bbox_pt,
            );
            validate_insets(
                errors,
                &format!("{path}.padding_pt"),
                cell.padding_pt,
                false,
            );
            validate_borders(errors, &format!("{path}.borders"), cell.borders);
            if let Some(fill) = &cell.fill {
                validate_fill(errors, &format!("{path}.fill"), fill);
            }
            validate_overflow(errors, path, cell.overflow);
        }
        Region::Image(image) => {
            validate_relationship(errors, &format!("{path}.relationship"), &image.relationship);
            if let Some(size) = image.intrinsic_size_px {
                validate_positive(
                    errors,
                    format!("{path}.intrinsic_size_px.width"),
                    size.width,
                );
                validate_positive(
                    errors,
                    format!("{path}.intrinsic_size_px.height"),
                    size.height,
                );
            }
            if let Some(crop) = image.crop {
                validate_normalized_insets(errors, &format!("{path}.crop"), crop);
            }
            validate_placement(errors, &format!("{path}.placement"), &image.placement);
            validate_unit(errors, format!("{path}.opacity"), image.opacity);
        }
        Region::Shape(shape) => {
            validate_path(errors, &format!("{path}.path"), &shape.path);
            validate_fill(errors, &format!("{path}.fill"), &shape.fill);
            if let Some(stroke) = &shape.stroke {
                validate_stroke(errors, &format!("{path}.stroke"), stroke);
            }
            validate_unit(errors, format!("{path}.opacity"), shape.opacity);
            validate_placement(errors, &format!("{path}.placement"), &shape.placement);
        }
    }
}

fn validate_region_graph(
    errors: &mut Vec<ValidationError>,
    regions: &HashMap<&RegionId, (usize, usize, &Region)>,
) {
    for (id, (page, position, region)) in regions {
        let path = format!("pages[{page}].regions[{position}]");
        if let Some(parent_id) = &region.common().parent_id {
            let Some((_, _, parent)) = regions.get(parent_id) else {
                errors.push(error(
                    format!("{path}.parent_id"),
                    "must reference a region",
                ));
                continue;
            };
            if !valid_parent(parent, region) {
                errors.push(error(
                    format!("{path}.parent_id"),
                    "references an incompatible composition parent",
                ));
            }
        } else if !matches!(region, Region::Story(_)) {
            errors.push(error(
                format!("{path}.parent_id"),
                "non-story regions must have a composition parent",
            ));
        }
        if let Some(owner_id) = &region.common().owner_id {
            if owner_id == *id {
                errors.push(error(
                    format!("{path}.owner_id"),
                    "must not reference itself",
                ));
            } else if !regions.contains_key(owner_id) {
                errors.push(error(format!("{path}.owner_id"), "must reference a region"));
            }
        }
        let mut seen = HashSet::new();
        let mut current = Some(*id);
        while let Some(current_id) = current {
            if !seen.insert(current_id) {
                errors.push(error(format!("{path}.parent_id"), "must not form a cycle"));
                break;
            }
            current = regions
                .get(current_id)
                .and_then(|(_, _, value)| value.common().parent_id.as_ref());
        }
    }
    for (id, (page, position, region)) in regions {
        let path = format!("pages[{page}].regions[{position}]");
        match region {
            Region::Story(story) => {
                for (index, linked) in story.linked_from.iter().enumerate() {
                    if !regions.contains_key(linked) {
                        errors.push(error(
                            format!("{path}.linked_from[{index}]"),
                            "must reference a region",
                        ));
                    }
                }
            }
            Region::Paragraph(paragraph) => {
                for (index, inline) in paragraph.content.iter().enumerate() {
                    let target = match &inline.kind {
                        InlineKind::NoteReference { target_story_id }
                        | InlineKind::CommentReference { target_story_id }
                        | InlineKind::Object {
                            target_region_id: target_story_id,
                        } => Some(target_story_id),
                        _ => None,
                    };
                    if let Some(target) = target {
                        let Some((_, _, target_region)) = regions.get(target) else {
                            errors.push(error(
                                format!("{path}.content[{index}]"),
                                "target must reference a region",
                            ));
                            continue;
                        };
                        let correct_kind = matches!(
                            (&inline.kind, target_region),
                            (
                                InlineKind::NoteReference { .. },
                                Region::Story(StoryRegion {
                                    story_kind: StoryKind::Footnote | StoryKind::Endnote,
                                    ..
                                }),
                            ) | (
                                InlineKind::CommentReference { .. },
                                Region::Story(StoryRegion {
                                    story_kind: StoryKind::Comment,
                                    ..
                                }),
                            ) | (InlineKind::Object { .. }, _)
                        );
                        if !correct_kind {
                            errors.push(error(
                                format!("{path}.content[{index}]"),
                                "target has the wrong story kind",
                            ));
                        }
                    }
                }
            }
            _ => {}
        }
        let _ = id;
    }
}

fn valid_parent(parent: &Region, child: &Region) -> bool {
    matches!(
        (parent, child),
        (
            Region::Shape(_),
            Region::Story(StoryRegion {
                story_kind: StoryKind::TextBox,
                ..
            })
        )
    ) || matches!(
        (parent, child),
        (Region::Story(_), Region::Paragraph(_))
            | (Region::Story(_), Region::Table(_))
            | (Region::Story(_), Region::Image(_))
            | (Region::Story(_), Region::Shape(_))
            | (Region::Table(_), Region::TableRow(_))
            | (Region::TableRow(_), Region::TableCell(_))
            | (Region::TableCell(_), Region::Paragraph(_))
            | (Region::TableCell(_), Region::Table(_))
            | (Region::TableCell(_), Region::Image(_))
            | (Region::TableCell(_), Region::Shape(_))
            | (Region::Paragraph(_), Region::Image(_))
            | (Region::Paragraph(_), Region::Shape(_))
    )
}

fn validate_reading_order(
    errors: &mut Vec<ValidationError>,
    regions: &HashMap<&RegionId, (usize, usize, &Region)>,
) {
    let mut groups = HashMap::<(usize, Option<&RegionId>), Vec<(usize, usize, usize)>>::new();
    for (page, position, region) in regions.values() {
        if let Some(order) = region.common().reading_order {
            groups
                .entry((*page, region.common().parent_id.as_ref()))
                .or_default()
                .push((order, *page, *position));
        }
    }
    for entries in groups.values_mut() {
        entries.sort_by_key(|entry| entry.0);
        for (expected, (actual, page, position)) in entries.iter().enumerate() {
            if *actual != expected {
                errors.push(error(
                    format!("pages[{page}].regions[{position}].reading_order"),
                    format!("must be contiguous from zero within its parent; expected {expected}"),
                ));
            }
        }
    }
}

fn validate_fragment_sequences(errors: &mut Vec<ValidationError>, document: &DocumentLayout) {
    struct Active {
        start_path: String,
        count: usize,
        next: usize,
        previous_page: usize,
        paragraph_text: Option<String>,
    }

    let mut active = HashMap::<(&'static str, &str, &str), Active>::new();
    for (page, layout) in document.pages.iter().enumerate() {
        for (position, region) in layout.regions.iter().enumerate() {
            let (kind, fragment, paragraph_text) = match region {
                Region::Paragraph(value) => {
                    ("paragraph", value.fragment, Some(value.text.as_str()))
                }
                Region::Table(value) => ("table", value.fragment, None),
                Region::TableRow(value) => ("table_row", value.fragment, None),
                Region::TableCell(value) => ("table_cell", value.fragment, None),
                _ => continue,
            };
            if fragment.count == 0 || fragment.index >= fragment.count {
                continue;
            }
            let source = region.source();
            let key = (kind, source.part.as_str(), source.id.as_str());
            let path = format!("pages[{page}].regions[{position}]");
            if fragment.index == 0 {
                if let Some(previous) = active.remove(&key) {
                    errors.push(error(
                        format!("{path}.fragment.index"),
                        format!(
                            "starts a new occurrence before {} completed",
                            previous.start_path
                        ),
                    ));
                }
                if fragment.count > 1 {
                    active.insert(
                        key,
                        Active {
                            start_path: path,
                            count: fragment.count,
                            next: 1,
                            previous_page: page,
                            paragraph_text: paragraph_text.map(str::to_owned),
                        },
                    );
                }
                continue;
            }

            let Some(sequence) = active.get_mut(&key) else {
                errors.push(error(
                    format!("{path}.fragment.index"),
                    "must follow fragment zero for the same source occurrence",
                ));
                continue;
            };
            if fragment.index != sequence.next || fragment.count != sequence.count {
                errors.push(error(
                    format!("{path}.fragment"),
                    format!("must be fragment {} of {}", sequence.next, sequence.count),
                ));
            }
            if page <= sequence.previous_page {
                errors.push(error(
                    format!("{path}.fragment.index"),
                    "continuation fragments must appear on later pages",
                ));
            }
            if paragraph_text.is_some_and(|text| sequence.paragraph_text.as_deref() != Some(text)) {
                errors.push(error(
                    format!("{path}.text"),
                    "must match the logical text of fragment zero",
                ));
            }
            sequence.next = fragment.index.saturating_add(1);
            sequence.previous_page = page;
            if sequence.next == sequence.count {
                active.remove(&key);
            }
        }
    }
    for sequence in active.into_values() {
        errors.push(error(
            format!("{}.fragment.count", sequence.start_path),
            format!(
                "sequence ended before all {} fragments were present",
                sequence.count
            ),
        ));
    }
}

fn validate_paragraph(errors: &mut Vec<ValidationError>, path: &str, paragraph: &ParagraphRegion) {
    validate_fragment(errors, &format!("{path}.fragment"), paragraph.fragment);
    validate_rect(
        errors,
        &format!("{path}.frame_bbox_pt"),
        paragraph.frame_bbox_pt,
    );
    if let Some(rect) = paragraph.content_bbox_pt {
        validate_rect(errors, &format!("{path}.content_bbox_pt"), rect);
    }
    validate_paragraph_style(errors, &format!("{path}.style"), &paragraph.style);
    validate_overflow(errors, path, paragraph.overflow);

    let mut previous_line_end = 0;
    for (index, line) in paragraph.lines.iter().enumerate() {
        let line_path = format!("{path}.lines[{index}]");
        validate_range(
            errors,
            &format!("{line_path}.range_utf8"),
            line.range_utf8,
            &paragraph.text,
        );
        if line.range_utf8[0] < previous_line_end {
            errors.push(error(
                format!("{line_path}.range_utf8"),
                "must not overlap the previous line",
            ));
        }
        previous_line_end = line.range_utf8[1];
        validate_rect(errors, &format!("{line_path}.bbox_pt"), line.bbox_pt);
        validate_finite(
            errors,
            format!("{line_path}.baseline_y_pt"),
            line.baseline_y_pt,
        );
        validate_nonnegative(errors, format!("{line_path}.ascent_pt"), line.ascent_pt);
        validate_nonnegative(errors, format!("{line_path}.descent_pt"), line.descent_pt);
        if line.baseline_y_pt < line.bbox_pt.y || line.baseline_y_pt > line.bbox_pt.bottom() {
            errors.push(error(
                format!("{line_path}.baseline_y_pt"),
                "must lie within line bbox_pt",
            ));
        }
    }

    let mut previous_run_end = 0;
    for (index, run) in paragraph.runs.iter().enumerate() {
        let run_path = format!("{path}.runs[{index}]");
        validate_range(
            errors,
            &format!("{run_path}.range_utf8"),
            run.range_utf8,
            &paragraph.text,
        );
        if run.range_utf8[0] < previous_run_end {
            errors.push(error(
                format!("{run_path}.range_utf8"),
                "must not overlap the previous run",
            ));
        }
        previous_run_end = run.range_utf8[1];
        if let Some(source) = &run.source {
            validate_source(errors, &format!("{run_path}.source"), source);
        }
        validate_typography(errors, &format!("{run_path}.typography"), &run.typography);
    }

    for (index, inline) in paragraph.content.iter().enumerate() {
        let inline_path = format!("{path}.content[{index}]");
        validate_range(
            errors,
            &format!("{inline_path}.range_utf8"),
            inline.range_utf8,
            &paragraph.text,
        );
        if let Some(source) = &inline.source {
            validate_source(errors, &format!("{inline_path}.source"), source);
        }
        if let InlineKind::Field {
            instruction,
            result_range_utf8,
        } = &inline.kind
        {
            validate_nonempty(errors, format!("{inline_path}.instruction"), instruction);
            validate_range(
                errors,
                &format!("{inline_path}.result_range_utf8"),
                *result_range_utf8,
                &paragraph.text,
            );
            if result_range_utf8[0] < inline.range_utf8[0]
                || result_range_utf8[1] > inline.range_utf8[1]
            {
                errors.push(error(
                    format!("{inline_path}.result_range_utf8"),
                    "must be contained in range_utf8",
                ));
            }
        }
    }
}

fn validate_paragraph_style(
    errors: &mut Vec<ValidationError>,
    path: &str,
    style: &ResolvedParagraphStyle,
) {
    for (name, value) in [
        ("spacing_before_pt", style.spacing_before_pt),
        ("spacing_after_pt", style.spacing_after_pt),
    ] {
        validate_nonnegative(errors, format!("{path}.{name}"), value);
    }
    for (name, value) in [
        ("indent_start_pt", style.indent_start_pt),
        ("indent_end_pt", style.indent_end_pt),
        ("first_line_indent_pt", style.first_line_indent_pt),
    ] {
        validate_finite(errors, format!("{path}.{name}"), value);
    }
    match style.line_spacing {
        ResolvedLineSpacing::Auto { multiplier } => validate_positive(
            errors,
            format!("{path}.line_spacing.multiplier"),
            multiplier,
        ),
        ResolvedLineSpacing::Exact { value_pt } | ResolvedLineSpacing::AtLeast { value_pt } => {
            validate_positive(errors, format!("{path}.line_spacing.value_pt"), value_pt)
        }
    }
    validate_borders(errors, &format!("{path}.borders"), style.borders);
    if let Some(color) = style.shading {
        validate_color(errors, &format!("{path}.shading"), color);
    }
}

fn validate_typography(
    errors: &mut Vec<ValidationError>,
    path: &str,
    typography: &ResolvedTypography,
) {
    validate_nonempty(
        errors,
        format!("{path}.font.family"),
        &typography.font.family,
    );
    validate_optional_nonempty(
        errors,
        format!("{path}.font.postscript_name"),
        typography.font.postscript_name.as_deref(),
    );
    validate_positive(errors, format!("{path}.size_pt"), typography.size_pt);
    validate_color(errors, &format!("{path}.color"), typography.color);
    validate_finite(
        errors,
        format!("{path}.letter_spacing_pt"),
        typography.letter_spacing_pt,
    );
    validate_positive(
        errors,
        format!("{path}.horizontal_scale"),
        typography.horizontal_scale,
    );
    validate_finite(
        errors,
        format!("{path}.baseline_shift_pt"),
        typography.baseline_shift_pt,
    );
    for (name, decoration) in [
        ("underline", &typography.underline),
        ("strike", &typography.strike),
    ] {
        if let Some(decoration) = decoration {
            validate_color(errors, &format!("{path}.{name}.color"), decoration.color);
            if let Some(width) = decoration.width_pt {
                validate_positive(errors, format!("{path}.{name}.width_pt"), width);
            }
        }
    }
}

fn validate_geometry(errors: &mut Vec<ValidationError>, path: &str, geometry: &RegionGeometry) {
    validate_rect(errors, &format!("{path}.bbox_pt"), geometry.bbox_pt);
    validate_rect(
        errors,
        &format!("{path}.local_bbox_pt"),
        geometry.local_bbox_pt,
    );
    for (field, value) in [
        ("m11", geometry.local_to_parent.m11),
        ("m12", geometry.local_to_parent.m12),
        ("m21", geometry.local_to_parent.m21),
        ("m22", geometry.local_to_parent.m22),
        ("dx", geometry.local_to_parent.dx),
        ("dy", geometry.local_to_parent.dy),
    ] {
        validate_finite(errors, format!("{path}.local_to_parent.{field}"), value);
    }
    if geometry.local_to_parent.determinant() == 0.0 {
        errors.push(error(
            format!("{path}.local_to_parent"),
            "must be invertible",
        ));
    }
    if let Some(clip) = &geometry.clip {
        match clip {
            ClipGeometry::Rect { rect_pt } => {
                validate_rect(errors, &format!("{path}.clip.rect_pt"), *rect_pt)
            }
            ClipGeometry::Path { path: clip_path } => {
                validate_path(errors, &format!("{path}.clip.path"), clip_path)
            }
        }
    }
}

fn validate_role(errors: &mut Vec<ValidationError>, path: &str, role: &RoleAssignment) {
    if let SemanticRole::Heading { level } = role.role
        && level == 0
    {
        errors.push(error(
            format!("{path}.role.level"),
            "must be greater than zero",
        ));
    }
    match &role.provenance {
        RoleProvenance::SourceDeclared {
            source,
            declaration,
        } => {
            validate_source(errors, &format!("{path}.provenance.source"), source);
            validate_nonempty(
                errors,
                format!("{path}.provenance.declaration"),
                declaration,
            );
        }
        RoleProvenance::Inferred { rule } => {
            validate_nonempty(errors, format!("{path}.provenance.rule"), rule)
        }
    }
}

fn validate_fragment(errors: &mut Vec<ValidationError>, path: &str, fragment: PageFragment) {
    if fragment.count == 0 {
        errors.push(error(format!("{path}.count"), "must be greater than zero"));
    } else if fragment.index >= fragment.count {
        errors.push(error(format!("{path}.index"), "must be less than count"));
    }
}

fn validate_placement(errors: &mut Vec<ValidationError>, path: &str, placement: &ObjectPlacement) {
    if let ObjectPlacement::Floating { placement } = placement {
        validate_floating(errors, &format!("{path}.placement"), placement);
    }
}

fn validate_floating(errors: &mut Vec<ValidationError>, path: &str, floating: &FloatingPlacement) {
    validate_point(errors, &format!("{path}.offset_pt"), floating.offset_pt);
    validate_insets(
        errors,
        &format!("{path}.distance_from_text_pt"),
        floating.distance_from_text_pt,
        false,
    );
    if let Some(path_value) = &floating.wrap_polygon {
        validate_path(errors, &format!("{path}.wrap_polygon"), path_value);
    }
}

fn validate_fill(errors: &mut Vec<ValidationError>, path: &str, fill: &Fill) {
    match fill {
        Fill::None => {}
        Fill::Solid { color } => validate_color(errors, &format!("{path}.color"), *color),
        Fill::Gradient { gradient } => {
            if gradient.stops.is_empty() {
                errors.push(error(format!("{path}.gradient.stops"), "must not be empty"));
            }
            let mut previous_position = 0.0;
            for (index, stop) in gradient.stops.iter().enumerate() {
                validate_unit(
                    errors,
                    format!("{path}.gradient.stops[{index}].position"),
                    stop.position,
                );
                validate_color(
                    errors,
                    &format!("{path}.gradient.stops[{index}].color"),
                    stop.color,
                );
                if index > 0 && stop.position < previous_position {
                    errors.push(error(
                        format!("{path}.gradient.stops[{index}].position"),
                        "must not precede the previous stop",
                    ));
                }
                previous_position = stop.position;
            }
            if let GradientKind::Linear { angle_degrees } = gradient.kind {
                validate_finite(
                    errors,
                    format!("{path}.gradient.kind.angle_degrees"),
                    angle_degrees,
                );
            }
        }
        Fill::Image { relationship, crop } => {
            validate_relationship(errors, &format!("{path}.relationship"), relationship);
            if let Some(crop) = crop {
                validate_normalized_insets(errors, &format!("{path}.crop"), *crop);
            }
        }
        Fill::Pattern {
            foreground,
            background,
            preset,
        } => {
            validate_color(errors, &format!("{path}.foreground"), *foreground);
            validate_color(errors, &format!("{path}.background"), *background);
            validate_nonempty(errors, format!("{path}.preset"), preset);
        }
    }
}

fn validate_stroke(errors: &mut Vec<ValidationError>, path: &str, stroke: &Stroke) {
    validate_color(errors, &format!("{path}.color"), stroke.color);
    validate_positive(errors, format!("{path}.width_pt"), stroke.width_pt);
    for (index, value) in stroke.dash_pattern_pt.iter().copied().enumerate() {
        validate_positive(errors, format!("{path}.dash_pattern_pt[{index}]"), value);
    }
}

fn validate_path(errors: &mut Vec<ValidationError>, path: &str, value: &PathGeometry) {
    if value.commands.is_empty() {
        errors.push(error(format!("{path}.commands"), "must not be empty"));
    } else if !matches!(value.commands[0], PathCommand::MoveTo { .. }) {
        errors.push(error(
            format!("{path}.commands[0]"),
            "must begin with move_to",
        ));
    }
    for (index, command) in value.commands.iter().enumerate() {
        let command_path = format!("{path}.commands[{index}]");
        match command {
            PathCommand::MoveTo { point } | PathCommand::LineTo { point } => {
                validate_point(errors, &format!("{command_path}.point"), *point)
            }
            PathCommand::CubicTo {
                control1,
                control2,
                point,
            } => {
                validate_point(errors, &format!("{command_path}.control1"), *control1);
                validate_point(errors, &format!("{command_path}.control2"), *control2);
                validate_point(errors, &format!("{command_path}.point"), *point);
            }
            PathCommand::Close => {}
        }
    }
}

fn validate_relationship(
    errors: &mut Vec<ValidationError>,
    path: &str,
    relationship: &RelationshipRef,
) {
    if crate::source_identity::validate_part_name(&relationship.source_part).is_err() {
        errors.push(error(
            format!("{path}.source_part"),
            "must be a canonical package-relative OPC part name",
        ));
    }
    if crate::source_identity::validate_part_name(&relationship.target_part).is_err() {
        errors.push(error(
            format!("{path}.target_part"),
            "must be a canonical package-relative OPC part name",
        ));
    }
    if !crate::source_identity::is_xml_local_name(&relationship.id) {
        errors.push(error(
            format!("{path}.id"),
            "must be an XML NCName relationship ID",
        ));
    }
}

fn validate_source(errors: &mut Vec<ValidationError>, path: &str, source: &SourceRef) {
    if crate::source_identity::validate_part_name(&source.part).is_err() {
        errors.push(error(
            format!("{path}.part"),
            "must be a canonical package-relative OPC part name",
        ));
    }
    if source.id.trim().is_empty() {
        errors.push(error(format!("{path}.id"), "must not be empty"));
        return;
    }
    let valid = match source.identity {
        IdentityKind::ParaId => {
            source.id.len() == 8
                && source
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte))
        }
        IdentityKind::RelationshipId => crate::source_identity::is_xml_local_name(&source.id),
        IdentityKind::WordId => source
            .id
            .parse::<i64>()
            .is_ok_and(|value| source.id == value.to_string()),
        IdentityKind::DrawingId => source
            .id
            .parse::<u32>()
            .is_ok_and(|value| source.id == value.to_string()),
        IdentityKind::GeneratedPath => crate::source_identity::is_valid_structural_path(&source.id),
    };
    if !valid {
        errors.push(error(
            format!("{path}.id"),
            "does not match its identity kind",
        ));
    }
}

fn validate_range(errors: &mut Vec<ValidationError>, path: &str, range: [usize; 2], text: &str) {
    let [start, end] = range;
    if start > end || end > text.len() {
        errors.push(error(
            path,
            format!("must be an ordered range within text length {}", text.len()),
        ));
    } else if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
        errors.push(error(path, "must fall on UTF-8 code point boundaries"));
    }
}

fn validate_borders(errors: &mut Vec<ValidationError>, path: &str, borders: BoxBorders) {
    for (name, border) in [
        ("top", borders.top),
        ("right", borders.right),
        ("bottom", borders.bottom),
        ("left", borders.left),
    ] {
        if let Some(border) = border {
            validate_color(errors, &format!("{path}.{name}.color"), border.color);
            validate_nonnegative(errors, format!("{path}.{name}.width_pt"), border.width_pt);
            validate_nonnegative(errors, format!("{path}.{name}.space_pt"), border.space_pt);
        }
    }
}

fn validate_overflow(errors: &mut Vec<ValidationError>, path: &str, overflow: Option<Overflow>) {
    if let Some(overflow) = overflow {
        validate_nonnegative(
            errors,
            format!("{path}.overflow.past_content_area_pt"),
            overflow.past_content_area_pt,
        );
    }
}

fn validate_rect(errors: &mut Vec<ValidationError>, path: &str, rect: Rect) {
    validate_finite(errors, format!("{path}.x"), rect.x);
    validate_finite(errors, format!("{path}.y"), rect.y);
    validate_nonnegative(errors, format!("{path}.width"), rect.width);
    validate_nonnegative(errors, format!("{path}.height"), rect.height);
}

fn validate_point(errors: &mut Vec<ValidationError>, path: &str, point: Point) {
    validate_finite(errors, format!("{path}.x"), point.x);
    validate_finite(errors, format!("{path}.y"), point.y);
}

fn validate_insets(
    errors: &mut Vec<ValidationError>,
    path: &str,
    insets: EdgeInsets,
    normalized: bool,
) {
    for (name, value) in [
        ("top", insets.top),
        ("right", insets.right),
        ("bottom", insets.bottom),
        ("left", insets.left),
    ] {
        if normalized {
            validate_unit(errors, format!("{path}.{name}"), value);
        } else {
            validate_nonnegative(errors, format!("{path}.{name}"), value);
        }
    }
}

fn validate_normalized_insets(
    errors: &mut Vec<ValidationError>,
    path: &str,
    insets: NormalizedInsets,
) {
    for (name, value) in [
        ("top", insets.top),
        ("right", insets.right),
        ("bottom", insets.bottom),
        ("left", insets.left),
    ] {
        validate_unit(errors, format!("{path}.{name}"), value);
    }
    if insets.left + insets.right >= 1.0 {
        errors.push(error(path, "left and right crop must leave positive width"));
    }
    if insets.top + insets.bottom >= 1.0 {
        errors.push(error(
            path,
            "top and bottom crop must leave positive height",
        ));
    }
}

fn validate_color(errors: &mut Vec<ValidationError>, path: &str, color: RgbaColor) {
    validate_unit(errors, format!("{path}.alpha"), color.alpha);
}

fn validate_finite(errors: &mut Vec<ValidationError>, path: impl Into<String>, value: f32) {
    if !value.is_finite() {
        errors.push(error(path, "must be finite"));
    }
}

fn validate_nonnegative(errors: &mut Vec<ValidationError>, path: impl Into<String>, value: f32) {
    if !value.is_finite() || value < 0.0 {
        errors.push(error(path, "must be finite and non-negative"));
    }
}

fn validate_positive(errors: &mut Vec<ValidationError>, path: impl Into<String>, value: f32) {
    if !value.is_finite() || value <= 0.0 {
        errors.push(error(path, "must be finite and greater than zero"));
    }
}

fn validate_unit(errors: &mut Vec<ValidationError>, path: impl Into<String>, value: f32) {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        errors.push(error(path, "must be finite and between zero and one"));
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

fn error(path: impl Into<String>, message: impl Into<String>) -> ValidationError {
    ValidationError {
        path: path.into(),
        message: message.into(),
    }
}
