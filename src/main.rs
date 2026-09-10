use std::path::PathBuf;

use clap::{Parser, Subcommand};
use typsastra_docx_ir::{
    AnyDocumentLayout, CoverageStatus, DocumentLayout, ProcessingLimits, Region, read_any_document,
};

#[derive(Parser)]
#[command(
    name = "typsastra-docx-ir",
    version,
    about = "Inspect and validate Typsastra DOCX Layout IR"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print a compact summary of an existing IR document.
    Summary { input: PathBuf },
    /// Validate JSON shape and IR invariants, not layout completeness or design.
    Validate { input: PathBuf },
    /// Print a versioned JSON Schema or write it to a file.
    Schema {
        /// Format major version to print.
        #[arg(long, default_value_t = 1)]
        major: u16,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Command::Summary { input } => {
            let document = read_any_document(&input, &ProcessingLimits::default())?;
            document.validate()?;
            print!("{}", summary_any(&document));
        }
        Command::Validate { input } => {
            let document = read_any_document(&input, &ProcessingLimits::default())?;
            document.validate()?;
            println!("valid IR: {} {}", document.format(), document.version());
            println!("layout completeness and design quality: not verified");
        }
        Command::Schema { major, output } => {
            let schema = match major {
                1 => typsastra_docx_ir::json_schema(),
                2 => typsastra_docx_ir::v2::json_schema(),
                _ => {
                    return Err(format!(
                        "unsupported schema major version {major} (supported: 1, 2)"
                    )
                    .into());
                }
            };
            let json = serde_json::to_vec_pretty(&schema)?;
            if let Some(output) = output {
                std::fs::write(&output, &json)?;
                println!("wrote {}", output.display());
            } else {
                println!("{}", String::from_utf8(json)?);
            }
        }
    }
    Ok(())
}

fn summary_any(document: &AnyDocumentLayout) -> String {
    match document {
        AnyDocumentLayout::V1(document) => summary(document),
        AnyDocumentLayout::V2(document) => summary_v2(document),
    }
}

fn summary(document: &DocumentLayout) -> String {
    use std::fmt::Write;

    let mut regions = 0;
    let mut paragraphs = 0;
    let mut measured = 0;
    let mut flagged = 0;
    for region in document.pages.iter().flat_map(|page| &page.regions) {
        regions += 1;
        if let Region::Paragraph(paragraph) = region {
            paragraphs += 1;
            if let Some(overflow) = paragraph.overflow {
                measured += 1;
                if overflow.horizontal
                    || overflow.vertical
                    || overflow.clipped
                    || overflow.past_content_area_pt > 0.0
                {
                    flagged += 1;
                }
            }
        }
    }
    let mut output = format!(
        "format: {} {}\nsource: {}\nrecorded pages: {}\nrecorded regions: {regions}\nrecorded paragraph segments: {paragraphs}\nparagraph segments with overflow measurements: {measured}\nparagraph segments without overflow measurements: {}\nrecorded paragraph segments with overflow or clipping: {flagged}\n",
        document.format,
        document.version,
        document.source.path,
        document.pages.len(),
        paragraphs - measured,
    );
    let coverage = document.coverage.clone().unwrap_or_default();
    output.push_str("measurement coverage (producer-declared, not independently verified):\n");
    for (name, status) in [
        ("pagination", coverage.pagination),
        ("text_geometry", coverage.text_geometry),
        ("resolved_typography", coverage.resolved_typography),
        ("table_geometry", coverage.table_geometry),
        ("drawing_appearance", coverage.drawing_appearance),
        ("composition_hierarchy", coverage.composition_hierarchy),
        ("overflow", coverage.overflow),
    ] {
        let label = match status {
            CoverageStatus::Unknown => "unknown",
            CoverageStatus::Absent => "absent",
            CoverageStatus::Partial => "partial",
            CoverageStatus::Complete => "complete",
        };
        writeln!(output, "  {name}: {label}").expect("writing to a String cannot fail");
    }
    if regions == 0 {
        if document.pages.is_empty() {
            output.push_str("No page structure or content regions recorded.\n");
        } else {
            output
                .push_str("Page structure present. Content geometry absent from region records.\n");
        }
        output.push_str("Layout quality and overflow cannot be assessed.\n");
    }
    output.push_str("Layout completeness and design quality: not verified; zero recorded findings is not a pass.\n");
    output
}

fn summary_v2(document: &typsastra_docx_ir::v2::DocumentLayout) -> String {
    use std::fmt::Write;
    use typsastra_docx_ir::v2::{CoverageStatus, Region};

    let mut stories = 0;
    let mut paragraphs = 0;
    let mut tables = 0;
    let mut rows = 0;
    let mut cells = 0;
    let mut images = 0;
    let mut shapes = 0;
    let mut runs = 0;
    let mut transformed = 0;
    let mut clipped = 0;
    let mut measured_overflow = 0;
    let mut flagged_overflow = 0;
    for region in document.pages.iter().flat_map(|page| &page.regions) {
        let common = region.common();
        if common.geometry.local_to_parent != typsastra_docx_ir::v2::AffineTransform::identity() {
            transformed += 1;
        }
        if common.geometry.clip.is_some() {
            clipped += 1;
        }
        let overflow = match region {
            Region::Story(_) => {
                stories += 1;
                None
            }
            Region::Paragraph(value) => {
                paragraphs += 1;
                runs += value.runs.len();
                value.overflow
            }
            Region::Table(value) => {
                tables += 1;
                value.overflow
            }
            Region::TableRow(value) => {
                rows += 1;
                value.overflow
            }
            Region::TableCell(value) => {
                cells += 1;
                value.overflow
            }
            Region::Image(_) => {
                images += 1;
                None
            }
            Region::Shape(_) => {
                shapes += 1;
                None
            }
        };
        if let Some(value) = overflow {
            measured_overflow += 1;
            if value.horizontal
                || value.vertical
                || value.clipped
                || value.past_content_area_pt > 0.0
            {
                flagged_overflow += 1;
            }
        }
    }
    let region_count = stories + paragraphs + tables + rows + cells + images + shapes;
    let mut output = format!(
        "format: {} {}\nsource: {}\nrecorded sections: {}\nrecorded pages: {}\nrecorded regions: {region_count}\nrecorded stories: {stories}\nrecorded paragraph fragments: {paragraphs}\nrecorded resolved runs: {runs}\nrecorded tables: {tables}\nrecorded table rows: {rows}\nrecorded table cells: {cells}\nrecorded images: {images}\nrecorded shapes: {shapes}\nregions with transforms: {transformed}\nregions with clips: {clipped}\nregions with overflow measurements: {measured_overflow}\nregions with overflow or clipping: {flagged_overflow}\n",
        document.format,
        document.version,
        document.source.path,
        document.sections.len(),
        document.pages.len(),
    );
    let coverage = document.coverage.clone().unwrap_or_default();
    output.push_str("measurement coverage (producer-declared, not independently verified):\n");
    for (name, status) in [
        ("pagination", coverage.pagination),
        ("text_geometry", coverage.text_geometry),
        ("resolved_typography", coverage.resolved_typography),
        ("table_geometry", coverage.table_geometry),
        ("drawing_appearance", coverage.drawing_appearance),
        ("composition_hierarchy", coverage.composition_hierarchy),
        ("overflow", coverage.overflow),
    ] {
        let label = match status {
            CoverageStatus::Unknown => "unknown",
            CoverageStatus::Absent => "absent",
            CoverageStatus::Partial => "partial",
            CoverageStatus::Complete => "complete",
        };
        writeln!(output, "  {name}: {label}").expect("writing to a String cannot fail");
    }
    output.push_str("Layout completeness and design quality: not verified; zero recorded findings is not a pass.\n");
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use typsastra_docx_ir::Overflow;

    fn example() -> DocumentLayout {
        serde_json::from_str(include_str!("../examples/minimal.docx-ir.json")).unwrap()
    }

    #[test]
    fn empty_six_page_ir_is_not_layout_verification() {
        let mut document = example();
        let mut page = document.pages[0].clone();
        page.regions.clear();
        document.pages = (0..6)
            .map(|index| {
                let mut page = page.clone();
                page.index = index;
                page
            })
            .collect();
        document.validate().unwrap();
        let text = summary(&document);
        assert!(text.contains("recorded pages: 6"));
        assert!(text.contains("Page structure present. Content geometry absent"));
        assert!(text.contains("Layout quality and overflow cannot be assessed."));
        assert!(text.contains("pagination: unknown"));
    }

    #[test]
    fn missing_overflow_is_not_a_negative_measurement() {
        let text = summary(&example());
        assert!(text.contains("paragraph segments without overflow measurements: 1"));
        assert!(text.contains("overflow: unknown"));
        assert!(text.contains("zero recorded findings is not a pass"));
    }

    #[test]
    fn clipping_and_content_area_excess_are_reported() {
        for overflow in [
            Overflow {
                clipped: true,
                ..Overflow::default()
            },
            Overflow {
                past_content_area_pt: 1.0,
                ..Overflow::default()
            },
            Overflow {
                horizontal: true,
                ..Overflow::default()
            },
            Overflow {
                vertical: true,
                ..Overflow::default()
            },
        ] {
            let mut document = example();
            let Region::Paragraph(paragraph) = &mut document.pages[0].regions[0] else {
                panic!("expected paragraph");
            };
            paragraph.overflow = Some(overflow);
            let text = summary(&document);
            assert!(text.contains("paragraph segments with overflow measurements: 1"));
            assert!(text.contains("recorded paragraph segments with overflow or clipping: 1"));
        }
    }

    #[test]
    fn declared_coverage_does_not_claim_verification() {
        let mut document = example();
        document.coverage = Some(typsastra_docx_ir::MeasurementCoverage {
            pagination: CoverageStatus::Complete,
            text_geometry: CoverageStatus::Partial,
            drawing_appearance: CoverageStatus::Absent,
            ..Default::default()
        });
        let text = summary(&document);
        assert!(text.contains("pagination: complete"));
        assert!(text.contains("text_geometry: partial"));
        assert!(text.contains("drawing_appearance: absent"));
        assert!(text.contains("resolved_typography: unknown"));
        assert!(text.contains("not independently verified"));
    }
}
