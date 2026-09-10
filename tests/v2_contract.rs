use typsastra_docx_ir::{ProcessingLimits, v2::*};

fn source(id: &str) -> SourceRef {
    SourceRef {
        part: "word/document.xml".into(),
        id: id.into(),
        identity: IdentityKind::ParaId,
    }
}

fn rect() -> Rect {
    Rect {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 20.0,
    }
}

fn common(id: &str, parent: Option<&str>, reading_order: usize) -> RegionCommon {
    RegionCommon {
        id: RegionId(id.into()),
        parent_id: parent.map(|value| RegionId(value.into())),
        owner_id: None,
        reading_order: Some(reading_order),
        role: None,
        geometry: RegionGeometry {
            bbox_pt: rect(),
            local_bbox_pt: rect(),
            local_to_parent: AffineTransform::identity(),
            clip: None,
        },
    }
}

fn fragment() -> PageFragment {
    PageFragment {
        index: 0,
        count: 1,
        continued_from_previous: false,
        continues_on_next: false,
    }
}

fn paragraph_style() -> ResolvedParagraphStyle {
    ResolvedParagraphStyle {
        alignment: ParagraphAlignment::Start,
        direction: TextDirection::LeftToRight,
        spacing_before_pt: 0.0,
        spacing_after_pt: 0.0,
        line_spacing: ResolvedLineSpacing::Auto { multiplier: 1.0 },
        indent_start_pt: 0.0,
        indent_end_pt: 0.0,
        first_line_indent_pt: 0.0,
        borders: BoxBorders::default(),
        shading: None,
        keep_next: false,
        keep_lines: false,
        widow_control: true,
        page_break_before: false,
    }
}

fn comprehensive_document() -> DocumentLayout {
    let story = Region::Story(StoryRegion {
        common: common("story", None, 0),
        source: source("00000001"),
        story_kind: StoryKind::Body,
        occurrence: 0,
        linked_from: Vec::new(),
    });
    let table = Region::Table(TableRegion {
        common: common("table", Some("story"), 0),
        source: source("00000002"),
        fragment: fragment(),
        content_bbox_pt: rect(),
        alignment: TableAlignment::Start,
        layout: TableLayoutMode::Fixed,
        cell_spacing_pt: 0.0,
        borders: BoxBorders::default(),
        floating: None,
        overflow: Some(Overflow::default()),
    });
    let row = Region::TableRow(TableRowRegion {
        common: common("row", Some("table"), 0),
        source: source("00000003"),
        fragment: fragment(),
        row_index: 0,
        repeated_header: true,
        cannot_split: false,
        grid_before: 0,
        overflow: Some(Overflow::default()),
    });
    let cell = Region::TableCell(TableCellRegion {
        common: common("cell", Some("row"), 0),
        source: source("00000004"),
        fragment: fragment(),
        row_index: 0,
        column_index: 0,
        grid_span: 2,
        row_span: 1,
        content_bbox_pt: rect(),
        padding_pt: EdgeInsets::default(),
        vertical_alignment: CellVerticalAlignment::Top,
        text_direction: TextDirection::LeftToRight,
        borders: BoxBorders::default(),
        fill: Some(Fill::Solid {
            color: RgbaColor {
                red: 240,
                green: 240,
                blue: 240,
                alpha: 1.0,
            },
        }),
        overflow: Some(Overflow::default()),
    });
    let paragraph = Region::Paragraph(ParagraphRegion {
        common: common("paragraph", Some("cell"), 0),
        source: source("00000005"),
        fragment: fragment(),
        text: "A\t1".into(),
        frame_bbox_pt: rect(),
        content_bbox_pt: Some(rect()),
        style: paragraph_style(),
        lines: vec![LineLayout {
            range_utf8: [0, 3],
            bbox_pt: rect(),
            baseline_y_pt: 10.0,
            ascent_pt: 8.0,
            descent_pt: 2.0,
        }],
        runs: vec![ResolvedRun {
            source: None,
            range_utf8: [0, 3],
            typography: ResolvedTypography {
                font: ResolvedFont {
                    family: "Aptos".into(),
                    postscript_name: Some("Aptos".into()),
                    face_index: Some(0),
                },
                size_pt: 11.0,
                color: RgbaColor {
                    red: 0,
                    green: 0,
                    blue: 0,
                    alpha: 1.0,
                },
                bold: false,
                italic: false,
                underline: None,
                strike: None,
                letter_spacing_pt: 0.0,
                horizontal_scale: 1.0,
                baseline_shift_pt: 0.0,
            },
        }],
        content: vec![
            InlineContent {
                range_utf8: [0, 1],
                source: None,
                kind: InlineKind::Text,
            },
            InlineContent {
                range_utf8: [1, 2],
                source: None,
                kind: InlineKind::Tab {
                    alignment: Some(TabAlignment::Decimal),
                },
            },
            InlineContent {
                range_utf8: [2, 3],
                source: None,
                kind: InlineKind::Field {
                    instruction: "PAGE".into(),
                    result_range_utf8: [2, 3],
                },
            },
        ],
        overflow: Some(Overflow::default()),
    });
    let relationship = RelationshipRef {
        source_part: "word/document.xml".into(),
        id: "rId1".into(),
        target_part: "word/media/image1.png".into(),
    };
    let image = Region::Image(ImageRegion {
        common: common("image", Some("story"), 1),
        source: source("00000006"),
        relationship,
        intrinsic_size_px: Some(Size {
            width: 640.0,
            height: 480.0,
        }),
        crop: Some(NormalizedInsets {
            left: 0.1,
            ..NormalizedInsets::default()
        }),
        placement: ObjectPlacement::Inline,
        opacity: 1.0,
    });
    let mut shape_common = common("shape", Some("story"), 2);
    shape_common.role = Some(RoleAssignment {
        role: SemanticRole::Figure,
        provenance: RoleProvenance::Inferred {
            rule: "drawing_with_visible_geometry".into(),
        },
    });
    shape_common.geometry.local_to_parent = AffineTransform {
        m11: 0.0,
        m12: 1.0,
        m21: -1.0,
        m22: 0.0,
        dx: 100.0,
        dy: 0.0,
    };
    shape_common.geometry.clip = Some(ClipGeometry::Rect { rect_pt: rect() });
    let shape = Region::Shape(ShapeRegion {
        common: shape_common,
        source: source("00000007"),
        path: PathGeometry {
            fill_rule: FillRule::NonZero,
            commands: vec![
                PathCommand::MoveTo {
                    point: Point { x: 0.0, y: 0.0 },
                },
                PathCommand::LineTo {
                    point: Point { x: 10.0, y: 10.0 },
                },
                PathCommand::Close,
            ],
        },
        fill: Fill::Solid {
            color: RgbaColor {
                red: 10,
                green: 20,
                blue: 30,
                alpha: 0.5,
            },
        },
        stroke: Some(Stroke {
            color: RgbaColor {
                red: 0,
                green: 0,
                blue: 0,
                alpha: 1.0,
            },
            width_pt: 1.0,
            dash_pattern_pt: vec![2.0, 1.0],
            cap: LineCap::Butt,
            join: LineJoin::Miter,
        }),
        opacity: 0.75,
        placement: ObjectPlacement::Floating {
            placement: FloatingPlacement {
                horizontal_anchor: AnchorReference::Margin,
                vertical_anchor: AnchorReference::Paragraph,
                horizontal_alignment: Some(AnchorAlignment::Center),
                vertical_alignment: None,
                offset_pt: Point { x: 1.0, y: 2.0 },
                wrap: WrapMode::Tight {
                    side: WrapSide::Both,
                },
                wrap_polygon: None,
                distance_from_text_pt: EdgeInsets::default(),
                z_order: 1,
                behind_text: false,
                allow_overlap: true,
            },
        },
    });

    DocumentLayout::new(
        GeneratorInfo {
            name: "test".into(),
            version: "1".into(),
        },
        SourceInfo {
            path: "test.docx".into(),
            byte_length: 1,
        },
        vec![SectionLayout {
            id: SectionId("section".into()),
            source: source("00000008"),
            index: 0,
            page_range: [0, 1],
            break_kind: SectionBreakKind::NextPage,
            title_page: false,
            different_odd_even: false,
        }],
        vec![PageLayout {
            index: 0,
            number: Some(1),
            size_pt: Size {
                width: 612.0,
                height: 792.0,
            },
            orientation: PageOrientation::Portrait,
            section_geometries: vec![SectionPageGeometry {
                section_id: SectionId("section".into()),
                margins_pt: EdgeInsets {
                    top: 72.0,
                    right: 72.0,
                    bottom: 72.0,
                    left: 72.0,
                },
                gutter_pt: 0.0,
                usable_content_bbox_pt: Rect {
                    x: 72.0,
                    y: 72.0,
                    width: 468.0,
                    height: 648.0,
                },
                columns: vec![ColumnLayout {
                    index: 0,
                    bbox_pt: rect(),
                }],
            }],
            regions: vec![story, table, row, cell, paragraph, image, shape],
        }],
    )
}

#[test]
fn comprehensive_v2_regions_validate_and_round_trip_canonically() {
    let document = comprehensive_document();
    document.validate().unwrap();
    let stats = document.stats();
    assert_eq!(stats.pages, 1);
    assert_eq!(stats.regions, 7);
    assert_eq!(stats.lines, 1);
    document.check_limits(&ProcessingLimits::default()).unwrap();
    let limits = ProcessingLimits {
        max_regions: 6,
        ..ProcessingLimits::default()
    };
    assert_eq!(
        document.check_limits(&limits).unwrap_err().resource,
        "regions"
    );

    let bytes = to_canonical_json(&document).unwrap();
    let restored: DocumentLayout = serde_json::from_slice(&bytes).unwrap();
    restored.validate().unwrap();
    assert_eq!(to_canonical_json(&restored).unwrap(), bytes);
    let text = String::from_utf8(bytes).unwrap();
    for kind in [
        "story",
        "paragraph",
        "table",
        "table_row",
        "table_cell",
        "image",
        "shape",
    ] {
        assert!(text.contains(&format!("\"kind\":\"{kind}\"")));
    }
}

#[test]
fn continuous_sections_can_share_one_physical_page() {
    let mut document = comprehensive_document();
    document.sections.push(SectionLayout {
        id: SectionId("section-continued".into()),
        source: source("00000009"),
        index: 1,
        page_range: [0, 1],
        break_kind: SectionBreakKind::Continuous,
        title_page: false,
        different_odd_even: false,
    });
    let mut geometry = document.pages[0].section_geometries[0].clone();
    geometry.section_id = SectionId("section-continued".into());
    geometry.usable_content_bbox_pt.y = 200.0;
    document.pages[0].section_geometries.push(geometry);
    document.validate().unwrap();
}

#[test]
fn validation_rejects_broken_hierarchy_ranges_and_transforms() {
    let mut document = comprehensive_document();
    document.pages[0].regions[1].common_mut_for_test().parent_id = Some(RegionId("missing".into()));
    let Region::Paragraph(paragraph) = &mut document.pages[0].regions[4] else {
        panic!("paragraph")
    };
    paragraph.lines[0].range_utf8 = [0, 2];
    paragraph.lines[0].baseline_y_pt = 30.0;
    let Region::Shape(shape) = &mut document.pages[0].regions[6] else {
        panic!("shape")
    };
    shape.common.geometry.local_to_parent.m22 = 0.0;
    shape.common.geometry.local_to_parent.m21 = 0.0;

    let errors = document.validate().unwrap_err().to_string();
    assert!(errors.contains("parent_id"));
    assert!(errors.contains("UTF-8 code point boundaries") || errors.contains("baseline_y_pt"));
    assert!(errors.contains("invertible"));
}

trait RegionTestExt {
    fn common_mut_for_test(&mut self) -> &mut RegionCommon;
}

impl RegionTestExt for Region {
    fn common_mut_for_test(&mut self) -> &mut RegionCommon {
        match self {
            Region::Story(value) => &mut value.common,
            Region::Paragraph(value) => &mut value.common,
            Region::Table(value) => &mut value.common,
            Region::TableRow(value) => &mut value.common,
            Region::TableCell(value) => &mut value.common,
            Region::Image(value) => &mut value.common,
            Region::Shape(value) => &mut value.common,
        }
    }
}
