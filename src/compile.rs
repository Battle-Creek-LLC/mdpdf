use std::collections::HashMap;

use anyhow::{bail, Context, Result};
use typst::foundations::{Bytes, Datetime};
use typst::layout::PagedDocument;
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

use crate::convert::Asset;
use crate::fonts;

/// A header or footer, as three already-converted Typst markup zones.
#[derive(Default, Clone)]
pub struct Zones {
    pub left: Option<String>,
    pub center: Option<String>,
    pub right: Option<String>,
    pub image_height_mm: f64,
    pub font_size: f64,
}

impl Zones {
    pub fn is_empty(&self) -> bool {
        self.left.is_none() && self.center.is_none() && self.right.is_none()
    }
}

pub struct CompileOptions {
    pub page_size: String,
    pub margin_mm: f64,
    pub font_size: f64,
    pub title: Option<String>,
    pub header: Zones,
    pub footer: Zones,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            page_size: "us-letter".to_string(),
            margin_mm: 25.0,
            font_size: 10.0,
            title: None,
            header: Zones::default(),
            footer: Zones::default(),
        }
    }
}

/// Replaces the header/footer tokens with Typst code.
///
/// Runs on already-escaped converter output: a literal `{page}` typed into body
/// text is escaped to inert characters, and only zone markup reaches this
/// function, so document text can never introduce a counter or a date.
pub fn substitute_tokens(markup: &str, title: Option<&str>) -> String {
    let title_text = title
        .map(|t| {
            t.chars()
                .flat_map(|ch| match ch {
                    '*' | '_' | '`' | '#' | '<' | '>' | '@' | '$' | '\\' | '~' | '[' | ']' => {
                        vec!['\\', ch]
                    }
                    _ => vec![ch],
                })
                .collect::<String>()
        })
        .unwrap_or_default();

    markup
        .replace("{page}", "#context counter(page).display()")
        .replace("{pages}", "#context counter(page).final().first()")
        .replace("{date}", "#datetime.today().display(\"[year]-[month]-[day]\")")
        .replace("{title}", &title_text)
}

pub fn compile_to_pdf(
    typst_body: &str,
    options: &CompileOptions,
    assets: &[Asset],
) -> Result<Vec<u8>> {
    let preamble = build_preamble(options);
    let full_source = format!("{}\n{}", preamble, typst_body);

    let mut asset_bytes = HashMap::new();
    for asset in assets {
        let data = std::fs::read(&asset.real_path)
            .with_context(|| format!("Failed to read image {}", asset.real_path.display()))?;
        // Key by the rootless form, which is what VirtualPath yields on lookup.
        let key = asset.virtual_path.trim_start_matches('/').to_string();
        asset_bytes.insert(key, Bytes::new(data));
    }

    let world = MdpdfWorld::new(full_source, asset_bytes);

    let result = typst::compile::<PagedDocument>(&world);

    let document = match result.output {
        Ok(doc) => doc,
        Err(diagnostics) => {
            let msgs: Vec<String> = diagnostics
                .iter()
                .map(|d| d.message.to_string())
                .collect();
            bail!("Typst compilation failed:\n{}", msgs.join("\n"));
        }
    };

    let pdf_options = typst_pdf::PdfOptions::default();
    match typst_pdf::pdf(&document, &pdf_options) {
        Ok(bytes) => Ok(bytes),
        Err(diagnostics) => {
            let msgs: Vec<String> = diagnostics
                .iter()
                .map(|d| d.message.to_string())
                .collect();
            bail!("PDF export failed:\n{}", msgs.join("\n"));
        }
    }
}

fn build_preamble(options: &CompileOptions) -> String {
    let title_line = match &options.title {
        Some(t) => format!(
            "#set document(title: \"{}\")\n",
            t.replace('\\', "\\\\").replace('"', "\\\"")
        ),
        None => String::new(),
    };

    let header_line = render_zones("header", &options.header);
    let footer_line = render_zones("footer", &options.footer);

    format!(
        r##"{title_line}#set page(paper: "{page}", margin: {margin}mm{header_line}{footer_line})
#set text(font: "Inter", size: {font_size}pt, fill: luma(30))
#set par(justify: true, leading: 0.65em, spacing: 0.65em)

#show heading.where(level: 1): set text(size: 21pt, weight: "bold")
#show heading.where(level: 2): set text(size: 17pt, weight: "bold")
#show heading.where(level: 3): set text(size: 14pt, weight: "bold")
#show heading.where(level: 4): set text(size: 12pt, weight: "bold")
#show heading.where(level: 5): set text(size: 11pt, weight: "bold")
#show heading.where(level: 6): set text(size: 11pt, weight: "bold")

#show heading: it => {{
  v(0.4em)
  it
  v(0.2em)
}}

#show raw.where(block: true): it => {{
  set text(font: "JetBrains Mono", size: 8pt)
  block(
    fill: luma(246),
    inset: 10pt,
    radius: 4pt,
    width: 100%,
    it,
  )
}}

#show raw.where(block: false): it => {{
  set text(font: "JetBrains Mono", size: 9pt)
  box(
    fill: luma(240),
    inset: (x: 3pt, y: 1pt),
    radius: 2pt,
    it,
  )
}}

#show link: it => {{
  set text(fill: rgb("#2563eb"))
  underline(offset: 2pt, stroke: 0.5pt + rgb("#93b4f5"), it)
}}

#let check-done = box(width: 10pt, height: 10pt, stroke: 0.5pt + luma(160), radius: 2pt, fill: rgb("#e8f5e9"), align(center + horizon, text(size: 7pt, fill: rgb("#2e7d32"), weight: "bold", "✓")))
#let check-todo = box(width: 10pt, height: 10pt, stroke: 0.5pt + luma(160), radius: 2pt)
"##,
        title_line = title_line,
        page = options.page_size,
        margin = options.margin_mm,
        header_line = header_line,
        footer_line = footer_line,
        font_size = options.font_size,
    )
}

/// Renders one band as a three-column grid so the left/center/right zones hold
/// their positions independently of how much content each one has.
fn render_zones(slot: &str, zones: &Zones) -> String {
    if zones.is_empty() {
        return String::new();
    }
    let cell = |zone: &Option<String>| match zone {
        Some(markup) => format!("[{}]", markup.trim()),
        None => "[]".to_string(),
    };

    format!(
        concat!(
            ",\n  {slot}: {{\n",
            "    set image(height: {image_height}mm)\n",
            "    set text(size: {font_size}pt, fill: luma(110))\n",
            "    grid(\n",
            "      columns: (1fr, 1fr, 1fr),\n",
            "      align: (left + horizon, center + horizon, right + horizon),\n",
            "      {left}, {center}, {right},\n",
            "    )\n",
            "  }}"
        ),
        slot = slot,
        image_height = zones.image_height_mm,
        font_size = zones.font_size,
        left = cell(&zones.left),
        center = cell(&zones.center),
        right = cell(&zones.right),
    )
}

struct MdpdfWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    main_source: Source,
    fonts: Vec<Font>,
    /// Images resolved ahead of compilation, keyed by rootless virtual path.
    /// Serving from this map rather than the real filesystem means a compile
    /// can only read files the converter already resolved.
    assets: HashMap<String, Bytes>,
}

impl MdpdfWorld {
    fn new(source: String, assets: HashMap<String, Bytes>) -> Self {
        let mut font_list = Vec::new();
        let mut book = FontBook::new();

        for font_bytes in fonts::ALL_FONTS {
            let buffer = Bytes::new(font_bytes.to_vec());
            for font in Font::iter(buffer) {
                book.push(font.info().clone());
                font_list.push(font);
            }
        }

        let main_id = FileId::new(None, VirtualPath::new("/main.typ"));
        let main_source = Source::new(main_id, source);

        Self {
            library: LazyHash::new(Library::default()),
            book: LazyHash::new(book),
            main_source,
            fonts: font_list,
            assets,
        }
    }
}

impl World for MdpdfWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.main_source.id()
    }

    fn source(&self, id: FileId) -> Result<Source, typst::diag::FileError> {
        if id == self.main_source.id() {
            Ok(self.main_source.clone())
        } else {
            Err(typst::diag::FileError::NotFound(
                id.vpath().as_rootless_path().into(),
            ))
        }
    }

    fn file(&self, id: FileId) -> Result<Bytes, typst::diag::FileError> {
        let path = id.vpath().as_rootless_path();
        match path.to_str().and_then(|key| self.assets.get(key)) {
            Some(bytes) => Ok(bytes.clone()),
            None => Err(typst::diag::FileError::NotFound(path.into())),
        }
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?;
        let days = (now.as_secs() / 86400) as i64;
        // Simple date calculation from epoch days
        let (y, m, d) = epoch_days_to_ymd(days);
        Datetime::from_ymd(y as i32, u8::try_from(m).ok()?, u8::try_from(d).ok()?)
    }
}

fn epoch_days_to_ymd(days: i64) -> (i64, i64, i64) {
    // Civil calendar algorithm from Howard Hinnant
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zones(left: Option<&str>, center: Option<&str>) -> Zones {
        Zones {
            left: left.map(String::from),
            center: center.map(String::from),
            right: None,
            image_height_mm: 8.0,
            font_size: 8.0,
        }
    }

    #[test]
    fn page_tokens_become_typst_counters() {
        let result = substitute_tokens("{page} of {pages}", None);
        assert!(result.contains("#context counter(page).display()"));
        assert!(result.contains("#context counter(page).final().first()"));
    }

    #[test]
    fn title_token_is_escaped() {
        // A title is document metadata a user controls; it must not be able to
        // introduce Typst markup through a header zone.
        let result = substitute_tokens("{title}", Some("#set page(width: 900cm)"));
        assert_eq!(result, "\\#set page(width: 900cm)");
    }

    #[test]
    fn title_token_with_no_title_renders_empty() {
        assert_eq!(substitute_tokens("{title}", None), "");
    }

    #[test]
    fn empty_band_emits_no_page_argument() {
        assert_eq!(render_zones("header", &Zones::default()), "");
    }

    #[test]
    fn band_emits_three_columns_with_empty_cells_held() {
        // Every zone must emit a cell even when unset, or a lone right-hand
        // zone would slide into the left column.
        let rendered = render_zones("footer", &zones(None, Some("mid")));
        assert!(rendered.contains("footer: {"));
        assert!(rendered.contains("columns: (1fr, 1fr, 1fr)"));
        assert!(rendered.contains("[], [mid], [],"));
    }

    #[test]
    fn band_constrains_image_height() {
        let rendered = render_zones("header", &zones(Some("#image(\"/assets/0.png\")"), None));
        assert!(rendered.contains("set image(height: 8mm)"));
    }

    #[test]
    fn preamble_omits_bands_when_unconfigured() {
        let preamble = build_preamble(&CompileOptions::default());
        assert!(preamble.contains("#set page(paper: \"us-letter\", margin: 25mm)"));
        assert!(!preamble.contains("header:"));
        assert!(!preamble.contains("footer:"));
    }

    #[test]
    fn preamble_includes_configured_band() {
        let options = CompileOptions {
            header: zones(Some("left bit"), None),
            ..CompileOptions::default()
        };
        let preamble = build_preamble(&options);
        assert!(preamble.contains("header: {"));
        assert!(preamble.contains("[left bit]"));
    }

    #[test]
    fn document_title_quote_is_escaped() {
        let options = CompileOptions {
            title: Some("a \" b".to_string()),
            ..CompileOptions::default()
        };
        assert!(build_preamble(&options).contains("\\\""));
    }
}
