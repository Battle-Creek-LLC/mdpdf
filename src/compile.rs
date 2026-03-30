use anyhow::{bail, Result};
use typst::foundations::{Bytes, Datetime};
use typst::layout::PagedDocument;
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

use crate::fonts;

pub struct CompileOptions {
    pub page_size: String,
    pub margin_mm: f64,
    pub font_size: f64,
    pub title: Option<String>,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            page_size: "us-letter".to_string(),
            margin_mm: 25.0,
            font_size: 10.0,
            title: None,
        }
    }
}

pub fn compile_to_pdf(typst_body: &str, options: &CompileOptions) -> Result<Vec<u8>> {
    let preamble = build_preamble(options);
    let full_source = format!("{}\n{}", preamble, typst_body);

    let world = MdpdfWorld::new(full_source);

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

    format!(
        r##"{title_line}#set page(paper: "{page}", margin: {margin}mm)
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
        font_size = options.font_size,
    )
}

struct MdpdfWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    main_source: Source,
    fonts: Vec<Font>,
}

impl MdpdfWorld {
    fn new(source: String) -> Self {
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
        Err(typst::diag::FileError::NotFound(
            id.vpath().as_rootless_path().into(),
        ))
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
