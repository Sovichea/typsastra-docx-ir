use std::path::PathBuf;

use clap::{Parser, Subcommand};
use typsastra_docx_ir::{ProcessingLimits, Region, json_schema, read_document};

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
    /// Validate JSON shape and semantic layout invariants.
    Validate { input: PathBuf },
    /// Print the v1 JSON Schema or write it to a file.
    Schema {
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Command::Summary { input } => {
            let document = read_document(&input, &ProcessingLimits::default())?;
            document.validate()?;
            let regions: usize = document.pages.iter().map(|page| page.regions.len()).sum();
            let overflows = document
                .pages
                .iter()
                .flat_map(|page| &page.regions)
                .filter(|region| match region {
                    Region::Paragraph(paragraph) => paragraph
                        .overflow
                        .is_some_and(|value| value.horizontal || value.vertical),
                    Region::Image(_) => false,
                })
                .count();
            println!("format: {} {}", document.format, document.version);
            println!("source: {}", document.source.path);
            println!("pages: {}", document.pages.len());
            println!("regions: {regions}");
            println!("overflows: {overflows}");
        }
        Command::Validate { input } => {
            let document = read_document(&input, &ProcessingLimits::default())?;
            document.validate()?;
            println!("valid: {} {}", document.format, document.version);
        }
        Command::Schema { output } => {
            let json = serde_json::to_vec_pretty(&json_schema())?;
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
