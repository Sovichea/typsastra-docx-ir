use std::path::PathBuf;

use clap::{Parser, Subcommand};
use typsastra_docx_ir::{DocumentLayout, FORMAT};

#[derive(Parser)]
#[command(
    name = "typsastra-docx-ir",
    version,
    about = "Inspect Typsastra DOCX Layout IR"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print a compact summary of an existing IR document.
    Summary { input: PathBuf },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Command::Summary { input } => {
            let bytes = std::fs::read(&input)?;
            let document: DocumentLayout = serde_json::from_slice(&bytes)?;
            if !document.is_supported() {
                return Err(format!(
                    "unsupported format: {} {} (expected {FORMAT} 1.x)",
                    document.format, document.version
                )
                .into());
            }
            let regions: usize = document.pages.iter().map(|page| page.regions.len()).sum();
            let overflows = document
                .pages
                .iter()
                .flat_map(|page| &page.regions)
                .filter(|region| match region {
                    typsastra_docx_ir::Region::Paragraph(paragraph) => paragraph
                        .overflow
                        .is_some_and(|value| value.horizontal || value.vertical),
                    typsastra_docx_ir::Region::Image(_) => false,
                })
                .count();
            println!("format: {} {}", document.format, document.version);
            println!("source: {}", document.source.path);
            println!("pages: {}", document.pages.len());
            println!("regions: {regions}");
            println!("overflows: {overflows}");
        }
    }
    Ok(())
}
