mod compile;
mod convert;
mod fonts;

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use clap::Parser;

use compile::CompileOptions;

#[derive(Parser)]
#[command(name = "mdpdf", version, about = "Convert Markdown to beautifully typeset PDFs")]
struct Cli {
    /// Markdown file paths (use "-" for stdin)
    #[arg(required = true)]
    inputs: Vec<String>,

    /// Output PDF path [default: <input-stem>.pdf, or combined.pdf with --combine].
    /// With multiple inputs, --output is only valid alongside --combine.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Combine all inputs into a single PDF, with a page break between each document
    #[arg(short = 'c', long)]
    combine: bool,

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

    /// Open each generated PDF in the system's default viewer
    #[arg(long)]
    open: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.output.is_some() && cli.inputs.len() > 1 && !cli.combine {
        anyhow::bail!("--output with multiple inputs requires --combine");
    }

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

    if cli.combine {
        let mut parts: Vec<String> = Vec::with_capacity(cli.inputs.len());
        for input in &cli.inputs {
            let markdown = read_input(input)?;
            parts.push(convert::markdown_to_typst(&markdown));
        }
        let typst_markup = parts.join("\n#pagebreak(weak: true)\n");
        let output_path = cli
            .output
            .clone()
            .unwrap_or_else(|| PathBuf::from("combined.pdf"));
        write_pdf(&typst_markup, &options, &output_path, cli.open)?;
        return Ok(());
    }

    for input in &cli.inputs {
        let markdown = read_input(input)?;
        let output_path = cli.output.clone().unwrap_or_else(|| {
            if input == "-" {
                PathBuf::from("output.pdf")
            } else {
                PathBuf::from(input).with_extension("pdf")
            }
        });
        let typst_markup = convert::markdown_to_typst(&markdown);
        write_pdf(&typst_markup, &options, &output_path, cli.open)?;
    }

    Ok(())
}

fn write_pdf(typst_markup: &str, options: &CompileOptions, output_path: &Path, open: bool) -> Result<()> {
    let pdf_bytes = compile::compile_to_pdf(typst_markup, options)?;
    std::fs::write(output_path, &pdf_bytes)
        .with_context(|| format!("Failed to write PDF to {}", output_path.display()))?;
    eprintln!("{}", output_path.display());
    if open {
        if let Err(e) = open_path(output_path) {
            eprintln!("Warning: failed to open {}: {}", output_path.display(), e);
        }
    }
    Ok(())
}

fn open_path(path: &Path) -> Result<()> {
    let (program, args): (&str, &[&str]) = match std::env::consts::OS {
        "macos" => ("open", &[]),
        "windows" => ("cmd", &["/C", "start", ""]),
        _ => ("xdg-open", &[]),
    };
    Command::new(program)
        .args(args)
        .arg(path)
        .status()
        .with_context(|| format!("Failed to launch '{}'", program))?;
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
