mod compile;
mod convert;
mod fonts;

use std::io::Read;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use compile::CompileOptions;

#[derive(Parser)]
#[command(name = "mdpdf", version, about = "Convert Markdown to beautifully typeset PDFs")]
struct Cli {
    /// Markdown file path (use "-" for stdin)
    input: String,

    /// Output PDF path [default: <input-stem>.pdf]
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// PDF document title metadata
    #[arg(short, long)]
    title: Option<String>,

    /// Base body font size in points
    #[arg(long, default_value = "10")]
    font_size: f64,

    /// Page size: letter, a4
    #[arg(long, default_value = "letter")]
    page_size: String,

    /// Page margins in mm
    #[arg(long, default_value = "25")]
    margin: f64,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let markdown = read_input(&cli.input)?;

    let output_path = cli.output.unwrap_or_else(|| {
        if cli.input == "-" {
            PathBuf::from("output.pdf")
        } else {
            let p = PathBuf::from(&cli.input);
            p.with_extension("pdf")
        }
    });

    let page_paper = match cli.page_size.to_lowercase().as_str() {
        "letter" | "us-letter" => "us-letter",
        "a4" => "a4",
        other => {
            eprintln!("Warning: unknown page size '{}', using letter", other);
            "us-letter"
        }
    };

    let options = CompileOptions {
        page_size: page_paper.to_string(),
        margin_mm: cli.margin,
        font_size: cli.font_size,
        title: cli.title,
    };

    let typst_markup = convert::markdown_to_typst(&markdown);
    let pdf_bytes = compile::compile_to_pdf(&typst_markup, &options)?;

    std::fs::write(&output_path, &pdf_bytes)
        .with_context(|| format!("Failed to write PDF to {}", output_path.display()))?;

    eprintln!("{}", output_path.display());

    Ok(())
}

fn read_input(path: &str) -> Result<String> {
    if path == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("Failed to read from stdin")?;
        Ok(buf)
    } else {
        std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path))
    }
}
