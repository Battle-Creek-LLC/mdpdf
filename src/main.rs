mod compile;
mod config;
mod convert;
mod fonts;

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use clap::Parser;

use compile::{CompileOptions, Zones};
use convert::AssetRegistry;

const DEFAULT_FONT_SIZE_PT: f64 = 10.0;
const DEFAULT_MARGIN_MM: f64 = 25.0;
const DEFAULT_PAGE_SIZE: &str = "letter";

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

    /// Base body font size in points [default: 10]
    #[arg(long)]
    font_size: Option<f64>,

    /// Page size: letter, a4 [default: letter]
    #[arg(long)]
    page_size: Option<String>,

    /// Page margins in mm [default: 25]
    #[arg(long)]
    margin: Option<f64>,

    /// Config file with [header], [footer], and [page] sections.
    /// Overrides the user config and any discovered .mdpdf.toml.
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Ignore the user config and any discovered .mdpdf.toml
    #[arg(long)]
    no_config: bool,

    /// Open each generated PDF in the system's default viewer
    #[arg(long)]
    open: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.output.is_some() && cli.inputs.len() > 1 && !cli.combine {
        anyhow::bail!("--output with multiple inputs requires --combine");
    }

    let cfg = config::load(&config_start_dir(&cli.inputs), cli.config.as_deref(), cli.no_config)?;

    // CLI flags win over config, which wins over the built-in defaults.
    let page_size = cli
        .page_size
        .clone()
        .or_else(|| cfg.page_size.clone())
        .unwrap_or_else(|| DEFAULT_PAGE_SIZE.to_string());
    let page_paper = match page_size.to_lowercase().as_str() {
        "letter" | "us-letter" => "us-letter",
        "a4" => "a4",
        other => {
            eprintln!("Warning: unknown page size '{}', using letter", other);
            "us-letter"
        }
    };

    let title = cli.title.clone().or_else(|| cfg.title.clone());
    let base_options = CompileOptions {
        page_size: page_paper.to_string(),
        margin_mm: cli.margin.or(cfg.margin_mm).unwrap_or(DEFAULT_MARGIN_MM),
        font_size: cli
            .font_size
            .or(cfg.font_size_pt)
            .unwrap_or(DEFAULT_FONT_SIZE_PT),
        title: title.clone(),
        header: Zones::default(),
        footer: Zones::default(),
    };

    if cli.combine {
        // One registry per output PDF, so each document embeds only the images
        // it actually references.
        let mut registry = AssetRegistry::new();
        let mut parts: Vec<String> = Vec::with_capacity(cli.inputs.len());
        for input in &cli.inputs {
            let markdown = read_input(input)?;
            parts.push(convert::markdown_to_typst_with_assets(
                &markdown,
                input_base_dir(input).as_deref(),
                &mut registry,
            ));
        }
        let typst_markup = parts.join("\n#pagebreak(weak: true)\n");
        let options = CompileOptions {
            header: render_band(&cfg.header, title.as_deref(), &mut registry),
            footer: render_band(&cfg.footer, title.as_deref(), &mut registry),
            ..base_options
        };
        let output_path = cli
            .output
            .clone()
            .unwrap_or_else(|| PathBuf::from("combined.pdf"));
        write_pdf(&typst_markup, &options, &registry, &output_path, cli.open)?;
        return Ok(());
    }

    for input in &cli.inputs {
        let mut registry = AssetRegistry::new();
        let markdown = read_input(input)?;
        let output_path = cli.output.clone().unwrap_or_else(|| {
            if input == "-" {
                PathBuf::from("output.pdf")
            } else {
                PathBuf::from(input).with_extension("pdf")
            }
        });
        let typst_markup = convert::markdown_to_typst_with_assets(
            &markdown,
            input_base_dir(input).as_deref(),
            &mut registry,
        );
        let options = CompileOptions {
            header: render_band(&cfg.header, title.as_deref(), &mut registry),
            footer: render_band(&cfg.footer, title.as_deref(), &mut registry),
            page_size: base_options.page_size.clone(),
            title: base_options.title.clone(),
            ..base_options
        };
        write_pdf(&typst_markup, &options, &registry, &output_path, cli.open)?;
    }

    Ok(())
}

/// Converts a band's zones from Markdown to Typst and resolves its tokens.
fn render_band(band: &config::Band, title: Option<&str>, registry: &mut AssetRegistry) -> Zones {
    if band.is_empty() {
        return Zones::default();
    }
    let mut zone = |source: &Option<config::Zone>| -> Option<String> {
        source.as_ref().map(|z| {
            let markup =
                convert::markdown_to_typst_with_assets(&z.markdown, Some(&z.base), registry);
            compile::substitute_tokens(&markup, title)
        })
    };
    Zones {
        left: zone(&band.left),
        center: zone(&band.center),
        right: zone(&band.right),
        image_height_mm: band
            .image_height_mm
            .unwrap_or(config::DEFAULT_BAND_IMAGE_HEIGHT_MM),
        font_size: band.font_size_pt.unwrap_or(config::DEFAULT_BAND_FONT_SIZE_PT),
    }
}

/// Where the upward search for `.mdpdf.toml` starts: the directory of the first
/// real file input, or the working directory when everything comes from stdin.
fn config_start_dir(inputs: &[String]) -> PathBuf {
    inputs
        .iter()
        .find(|i| *i != "-")
        .and_then(|i| input_base_dir(i))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// The directory image paths in a document resolve against.
fn input_base_dir(input: &str) -> Option<PathBuf> {
    if input == "-" {
        // stdin has no directory of its own, so relative images resolve against
        // the working directory the user invoked from.
        return std::env::current_dir().ok();
    }
    let path = PathBuf::from(input);
    match path.parent() {
        Some(p) if p.as_os_str().is_empty() => Some(PathBuf::from(".")),
        Some(p) => Some(p.to_path_buf()),
        None => Some(PathBuf::from(".")),
    }
}

fn write_pdf(
    typst_markup: &str,
    options: &CompileOptions,
    registry: &AssetRegistry,
    output_path: &Path,
    open: bool,
) -> Result<()> {
    for warning in registry.warnings() {
        eprintln!("Warning: {}", warning);
    }
    let pdf_bytes = compile::compile_to_pdf(typst_markup, options, registry.assets())?;
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
