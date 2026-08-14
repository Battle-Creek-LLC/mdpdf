use std::path::{Path, PathBuf};

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Image formats Typst can decode. An unrecognized extension is reported rather
/// than passed through, because Typst infers the format from the path alone.
const SUPPORTED_IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "svg", "webp"];

/// One image resolved from Markdown, mapped to the virtual path the generated
/// Typst refers to it by.
pub struct Asset {
    pub virtual_path: String,
    pub real_path: PathBuf,
}

/// Collects the images referenced across every document and header zone in a
/// single render, so `MdpdfWorld` can serve them and indices never collide.
#[derive(Default)]
pub struct AssetRegistry {
    assets: Vec<Asset>,
    warnings: Vec<String>,
}

impl AssetRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn assets(&self) -> &[Asset] {
        &self.assets
    }

    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    fn warn(&mut self, message: String) {
        self.warnings.push(message);
    }

    /// Registers a resolved image and returns the virtual path to reference it
    /// by. Re-registering the same file — a logo repeated across documents —
    /// reuses the existing entry so the bytes are only held once.
    fn register(&mut self, real_path: PathBuf, ext: &str) -> String {
        if let Some(existing) = self.assets.iter().find(|a| a.real_path == real_path) {
            return existing.virtual_path.clone();
        }
        let virtual_path = format!("/assets/{}.{}", self.assets.len(), ext);
        self.assets.push(Asset {
            virtual_path: virtual_path.clone(),
            real_path,
        });
        virtual_path
    }
}

/// Converts with no base directory, so relative image paths do not resolve.
/// Callers in `main` always know the source directory and use
/// [`markdown_to_typst_with_assets`] directly.
#[cfg(test)]
pub fn markdown_to_typst(markdown: &str) -> String {
    let mut registry = AssetRegistry::new();
    markdown_to_typst_with_assets(markdown, None, &mut registry)
}

/// Converts Markdown to Typst markup, resolving image references against
/// `base` — the directory of the file the Markdown came from.
pub fn markdown_to_typst_with_assets(
    markdown: &str,
    base: Option<&Path>,
    registry: &mut AssetRegistry,
) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_GFM;
    let parser = Parser::new_ext(markdown, options);
    let mut converter = Converter::new(base, registry);
    converter.convert(parser);
    converter.output
}

/// Resolves a Markdown image destination to a readable local file.
///
/// Remote URLs are refused rather than fetched: running without network access
/// is a property of this tool, and a document converter that silently makes
/// requests would be a surprising capability.
fn resolve_image_path(dest: &str, base: Option<&Path>) -> Result<PathBuf, String> {
    let dest = dest.trim();
    if dest.is_empty() {
        return Err("empty image path".to_string());
    }
    if dest.starts_with("data:") {
        return Err("data: URIs are not supported".to_string());
    }
    if dest.contains("://") {
        return Err(format!("remote images are not fetched: {}", dest));
    }

    let expanded = match dest.strip_prefix("~/") {
        Some(rest) => match home_dir() {
            Some(home) => home.join(rest),
            None => return Err(format!("cannot expand '~' in {}: no home directory", dest)),
        },
        None => PathBuf::from(dest),
    };

    let resolved = if expanded.is_absolute() {
        expanded
    } else {
        match base {
            Some(dir) => dir.join(expanded),
            None => expanded,
        }
    };

    if !resolved.is_file() {
        return Err(format!("image not found: {}", resolved.display()));
    }
    Ok(resolved)
}

fn image_extension(path: &Path) -> Result<String, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .ok_or_else(|| format!("image has no file extension: {}", path.display()))?;
    if !SUPPORTED_IMAGE_EXTS.contains(&ext.as_str()) {
        return Err(format!(
            "unsupported image format '{}' ({}); supported: {}",
            ext,
            path.display(),
            SUPPORTED_IMAGE_EXTS.join(", ")
        ));
    }
    Ok(ext)
}

fn home_dir() -> Option<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var).map(PathBuf::from).filter(|p| !p.as_os_str().is_empty())
}

struct Converter<'a> {
    base: Option<PathBuf>,
    registry: &'a mut AssetRegistry,
    in_image: bool,
    image_alt: String,
    image_vpath: Option<String>,
    output: String,
    list_stack: Vec<ListKind>,
    in_code_block: bool,
    code_block_lang: Option<String>,
    code_block_buf: String,
    in_table: bool,
    table_alignments: Vec<pulldown_cmark::Alignment>,
    table_row_cells: Vec<String>,
    current_cell: String,
    in_table_head: bool,
    needs_paragraph_break: bool,
    inline_stack: Vec<InlineStyle>,
    in_block_quote: bool,
    block_quote_buf: String,
    in_block_quote_depth: usize,
    last_item_marker_end: Option<usize>,
}

#[derive(Clone)]
enum ListKind {
    Unordered,
    Ordered(u64),
}

#[derive(Clone, Copy)]
enum InlineStyle {
    Emphasis,
    Strong,
    Strikethrough,
}

impl<'a> Converter<'a> {
    fn new(base: Option<&Path>, registry: &'a mut AssetRegistry) -> Self {
        Self {
            base: base.map(PathBuf::from),
            registry,
            in_image: false,
            image_alt: String::new(),
            image_vpath: None,
            output: String::new(),
            list_stack: Vec::new(),
            in_code_block: false,
            code_block_lang: None,
            code_block_buf: String::new(),
            in_table: false,
            table_alignments: Vec::new(),
            table_row_cells: Vec::new(),
            current_cell: String::new(),
            in_table_head: false,
            needs_paragraph_break: false,
            inline_stack: Vec::new(),
            in_block_quote: false,
            block_quote_buf: String::new(),
            in_block_quote_depth: 0,
            last_item_marker_end: None,
        }
    }

    fn convert(&mut self, parser: Parser) {
        for event in parser {
            self.process_event(event);
        }
    }

    fn process_event(&mut self, event: Event) {
        match event {
            Event::Start(tag) => self.start_tag(tag),
            Event::End(tag) => self.end_tag(tag),
            Event::Text(text) => self.text(&text),
            Event::Code(code) => self.inline_code(&code),
            Event::SoftBreak => self.soft_break(),
            Event::HardBreak => self.hard_break(),
            Event::Rule => self.rule(),
            Event::TaskListMarker(checked) => self.task_list_marker(checked),
            _ => {}
        }
    }

    fn push(&mut self, s: &str) {
        if self.in_code_block {
            self.code_block_buf.push_str(s);
        } else if self.in_table {
            self.current_cell.push_str(s);
        } else if self.in_block_quote {
            self.block_quote_buf.push_str(s);
        } else {
            self.output.push_str(s);
        }
    }

    fn active_buf(&mut self) -> &mut String {
        if self.in_block_quote {
            &mut self.block_quote_buf
        } else {
            &mut self.output
        }
    }

    fn ensure_newline(&mut self) {
        let buf = self.active_buf();
        if !buf.ends_with('\n') {
            buf.push('\n');
        }
    }

    fn escape_typst(text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        for ch in text.chars() {
            match ch {
                '*' | '_' | '`' | '#' | '<' | '>' | '@' | '$' | '\\' | '~' | '[' | ']' => {
                    result.push('\\');
                    result.push(ch);
                }
                _ => result.push(ch),
            }
        }
        result
    }

    fn start_tag(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => {
                if self.needs_paragraph_break && !self.in_list() {
                    self.push("\n");
                }
            }
            Tag::Heading { level, .. } => {
                if self.needs_paragraph_break {
                    self.push("\n");
                }
                let marker = match level {
                    HeadingLevel::H1 => "= ",
                    HeadingLevel::H2 => "== ",
                    HeadingLevel::H3 => "=== ",
                    HeadingLevel::H4 => "==== ",
                    HeadingLevel::H5 => "===== ",
                    HeadingLevel::H6 => "====== ",
                };
                self.push(marker);
            }
            Tag::BlockQuote(_) => {
                if self.in_block_quote {
                    self.in_block_quote_depth += 1;
                } else {
                    if self.needs_paragraph_break {
                        self.output.push('\n');
                    }
                    self.in_block_quote = true;
                    self.in_block_quote_depth = 1;
                    self.block_quote_buf.clear();
                }
            }
            Tag::CodeBlock(kind) => {
                if self.needs_paragraph_break && !self.in_block_quote {
                    self.output.push('\n');
                }
                self.in_code_block = true;
                self.code_block_buf.clear();
                self.code_block_lang = match kind {
                    CodeBlockKind::Fenced(lang) => {
                        let lang = lang.trim().to_string();
                        if lang.is_empty() { None } else { Some(lang) }
                    }
                    CodeBlockKind::Indented => None,
                };
            }
            Tag::List(first_item) => {
                if self.list_stack.is_empty() && self.needs_paragraph_break {
                    self.push("\n");
                }
                match first_item {
                    Some(start) => self.list_stack.push(ListKind::Ordered(start)),
                    None => self.list_stack.push(ListKind::Unordered),
                }
            }
            Tag::Item => {
                self.ensure_newline();
                let indent = "  ".repeat(self.list_stack.len().saturating_sub(1));
                let marker = match self.list_stack.last() {
                    Some(ListKind::Unordered) => "- ",
                    Some(ListKind::Ordered(_)) => "+ ",
                    None => "- ",
                };
                if let Some(ListKind::Ordered(n)) = self.list_stack.last_mut() {
                    *n += 1;
                }
                self.push(&format!("{}{}", indent, marker));
                self.last_item_marker_end = Some(self.active_buf().len());
            }
            Tag::Emphasis => {
                self.inline_stack.push(InlineStyle::Emphasis);
                self.push("_");
            }
            Tag::Strong => {
                self.inline_stack.push(InlineStyle::Strong);
                self.push("*");
            }
            Tag::Strikethrough => {
                self.inline_stack.push(InlineStyle::Strikethrough);
                self.push("#strike[");
            }
            Tag::Link { dest_url, .. } => {
                self.push(&format!("#link(\"{}\")[", Self::escape_typst_string(&dest_url)));
            }
            Tag::Image { dest_url, .. } => {
                self.in_image = true;
                self.image_alt.clear();
                self.image_vpath = match resolve_image_path(&dest_url, self.base.as_deref())
                    .and_then(|path| image_extension(&path).map(|ext| (path, ext)))
                {
                    Ok((path, ext)) => Some(self.registry.register(path, &ext)),
                    Err(reason) => {
                        self.registry.warn(reason);
                        None
                    }
                };
            }
            Tag::Table(alignments) => {
                if self.needs_paragraph_break {
                    self.push("\n");
                }
                self.in_table = true;
                self.table_alignments = alignments;
                self.table_row_cells.clear();

                let cols = self.table_alignments.len();
                let align_strs: Vec<&str> = self.table_alignments.iter().map(|a| {
                    match a {
                        pulldown_cmark::Alignment::Left => "left",
                        pulldown_cmark::Alignment::Center => "center",
                        pulldown_cmark::Alignment::Right => "right",
                        pulldown_cmark::Alignment::None => "left",
                    }
                }).collect();

                let header = format!(
                    "#table(\n  columns: {},\n  align: ({}),\n  stroke: 0.5pt + luma(200),\n  inset: 8pt,\n",
                    cols,
                    align_strs.join(", "),
                );
                self.active_buf().push_str(&header);
            }
            Tag::TableHead => {
                self.in_table_head = true;
                self.table_row_cells.clear();
            }
            Tag::TableRow => {
                self.table_row_cells.clear();
            }
            Tag::TableCell => {
                self.current_cell.clear();
            }
            _ => {}
        }
    }

    fn end_tag(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                self.push("\n");
                self.needs_paragraph_break = true;
            }
            TagEnd::Heading(_) => {
                self.push("\n");
                self.needs_paragraph_break = true;
            }
            TagEnd::BlockQuote(_) => {
                if self.in_block_quote_depth > 1 {
                    self.in_block_quote_depth -= 1;
                } else {
                    self.in_block_quote = false;
                    self.in_block_quote_depth = 0;
                    let content = std::mem::take(&mut self.block_quote_buf);
                    let trimmed = content.trim();
                    self.output.push_str(&format!(
                        "#block(inset: (left: 12pt), stroke: (left: 2pt + luma(180)))[{}]\n",
                        trimmed
                    ));
                    self.needs_paragraph_break = true;
                }
            }
            TagEnd::CodeBlock => {
                self.in_code_block = false;
                let code = std::mem::take(&mut self.code_block_buf);
                let lang = self.code_block_lang.take();

                let code = code.trim_end_matches('\n');
                let lang_tag = lang.as_deref().unwrap_or("");
                let fence = "`".repeat(Self::block_fence_len(code));

                let rendered = format!("{fence}{lang_tag}\n{code}\n{fence}\n");
                self.active_buf().push_str(&rendered);
                self.needs_paragraph_break = true;
            }
            TagEnd::List(_) => {
                self.list_stack.pop();
                if self.list_stack.is_empty() {
                    self.needs_paragraph_break = true;
                }
            }
            TagEnd::Item => {
                self.ensure_newline();
            }
            TagEnd::Emphasis => {
                self.inline_stack.pop();
                self.push("_");
            }
            TagEnd::Strong => {
                self.inline_stack.pop();
                self.push("*");
            }
            TagEnd::Strikethrough => {
                self.inline_stack.pop();
                self.push("]");
            }
            TagEnd::Link => {
                self.push("]");
            }
            TagEnd::Image => {
                self.in_image = false;
                let alt = std::mem::take(&mut self.image_alt);
                match self.image_vpath.take() {
                    // The virtual path is generated, never user text, so it
                    // cannot break out of the image() call.
                    Some(vpath) => {
                        let rendered = if alt.is_empty() {
                            format!("#image(\"{}\")", vpath)
                        } else {
                            format!(
                                "#image(\"{}\", alt: \"{}\")",
                                vpath,
                                Self::escape_typst_string(&alt)
                            )
                        };
                        self.push(&rendered);
                    }
                    // Unresolvable image: fall back to its alt text so the
                    // document still says what was meant to be there.
                    None => {
                        if !alt.is_empty() {
                            self.push(&Self::escape_typst(&alt));
                        }
                    }
                }
            }
            TagEnd::Table => {
                self.in_table = false;
                self.active_buf().push_str(")\n");
                self.needs_paragraph_break = true;
            }
            TagEnd::TableHead => {
                self.in_table_head = false;
                let cells: Vec<String> = self.table_row_cells
                    .iter()
                    .map(|c| format!("[*{}*]", c.trim()))
                    .collect();
                let line = format!("  table.header({}),\n", cells.join(", "));
                self.active_buf().push_str(&line);
                self.table_row_cells.clear();
            }
            TagEnd::TableRow => {
                if !self.in_table_head {
                    let rows: String = self
                        .table_row_cells
                        .iter()
                        .map(|cell| format!("  [{}],\n", cell.trim()))
                        .collect();
                    self.active_buf().push_str(&rows);
                }
                self.table_row_cells.clear();
            }
            TagEnd::TableCell => {
                let cell = std::mem::take(&mut self.current_cell);
                self.table_row_cells.push(cell);
            }
            _ => {}
        }
    }

    fn text(&mut self, text: &str) {
        if self.in_image {
            // Alt text is an attribute of the image, not document content.
            self.image_alt.push_str(text);
        } else if self.in_code_block {
            self.code_block_buf.push_str(text);
        } else {
            self.push(&Self::escape_typst(text));
        }
    }

    fn inline_code(&mut self, code: &str) {
        // Inline raw uses the shortest fence that can't be terminated by the
        // content. A minimum of 1 keeps simple snippets as `x` — Typst only
        // interprets the first word as a language tag when the fence is 3+
        // backticks, so a min of 3 would silently eat e.g. `let` from
        // `` `let x = 5` ``.
        let fence = "`".repeat(Self::inline_fence_len(code));
        let pad_left = if code.starts_with('`') { " " } else { "" };
        let pad_right = if code.ends_with('`') { " " } else { "" };
        self.push(&format!("{fence}{pad_left}{code}{pad_right}{fence}"));
    }

    fn inline_fence_len(content: &str) -> usize {
        (Self::max_backtick_run(content) + 1).max(1)
    }

    /// Block raw needs a minimum of 3 backticks so Typst parses the language
    /// tag; grow beyond that only if the body contains a backtick run of 3+.
    fn block_fence_len(content: &str) -> usize {
        (Self::max_backtick_run(content) + 1).max(3)
    }

    fn max_backtick_run(content: &str) -> usize {
        let mut max_run = 0usize;
        let mut cur = 0usize;
        for ch in content.chars() {
            if ch == '`' {
                cur += 1;
                if cur > max_run {
                    max_run = cur;
                }
            } else {
                cur = 0;
            }
        }
        max_run
    }

    fn soft_break(&mut self) {
        if self.in_code_block {
            self.code_block_buf.push('\n');
        } else {
            self.push("\n");
        }
    }

    fn hard_break(&mut self) {
        self.push(" \\\n");
    }

    fn rule(&mut self) {
        if self.needs_paragraph_break {
            self.push("\n");
        }
        self.push("#line(length: 100%, stroke: 0.5pt + luma(180))\n");
        self.needs_paragraph_break = true;
    }

    fn task_list_marker(&mut self, checked: bool) {
        let symbol = if checked { "#check-done " } else { "#check-todo " };
        if let Some(pos) = self.last_item_marker_end.take() {
            let buf = self.active_buf();
            if pos <= buf.len() {
                buf.insert_str(pos, symbol);
            }
        }
    }

    fn in_list(&self) -> bool {
        !self.list_stack.is_empty()
    }

    fn escape_typst_string(s: &str) -> String {
        s.replace('\\', "\\\\").replace('"', "\\\"")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heading() {
        let result = markdown_to_typst("# Hello World");
        assert!(result.contains("= Hello World"));
    }

    #[test]
    fn test_bold_italic() {
        let result = markdown_to_typst("**bold** and *italic*");
        assert!(result.contains("*bold*"));
        assert!(result.contains("_italic_"));
    }

    #[test]
    fn test_code_block() {
        let result = markdown_to_typst("```rust\nfn main() {}\n```");
        assert!(result.contains("```rust"));
        assert!(result.contains("fn main() {}"));
    }

    #[test]
    fn test_unordered_list() {
        let result = markdown_to_typst("- one\n- two\n- three");
        assert!(result.contains("- one\n"));
        assert!(result.contains("- two\n"));
        assert!(result.contains("- three\n"));
    }

    #[test]
    fn test_ordered_list() {
        let result = markdown_to_typst("1. first\n2. second\n3. third");
        assert!(result.contains("+ first\n"));
        assert!(result.contains("+ second\n"));
    }

    #[test]
    fn test_nested_list() {
        let result = markdown_to_typst("- one\n  - nested\n- two");
        assert!(result.contains("- one\n"));
        assert!(result.contains("  - nested\n"));
        assert!(result.contains("- two\n"));
    }

    #[test]
    fn test_link() {
        let result = markdown_to_typst("[click here](https://example.com)");
        assert!(result.contains("#link(\"https://example.com\")[click here]"));
    }

    #[test]
    fn test_escape_special_chars() {
        let result = markdown_to_typst("price is $10 and 5 < 10");
        assert!(result.contains("\\$10"));
        assert!(result.contains("\\<"));
    }

    #[test]
    fn test_table() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |";
        let result = markdown_to_typst(md);
        assert!(result.contains("#table("));
        assert!(result.contains("columns: 2"));
        assert!(result.contains("table.header([*A*], [*B*])"));
    }

    #[test]
    fn test_task_list() {
        let md = "- [x] done\n- [ ] todo";
        let result = markdown_to_typst(md);
        assert!(result.contains("#check-done"));
        assert!(result.contains("#check-todo"));
    }

    #[test]
    fn test_escape_brackets_in_text() {
        // Bare [ and ] in body text must be escaped so they cannot terminate
        // a surrounding Typst content block.
        let result = markdown_to_typst("an array like [1, 2, 3] here");
        assert!(result.contains("\\["));
        assert!(result.contains("\\]"));
    }

    #[test]
    fn test_strikethrough_bracket_injection_blocked() {
        // Without escaping, the inner ] would close the #strike[...] block
        // and let "#text(fill: red)[evil]" run as Typst markup.
        let result = markdown_to_typst("~~foo ] #text(fill: red)[evil~~");
        assert!(result.contains("#strike["));
        assert!(!result.contains("] #text(fill: red)["));
        assert!(result.contains("\\]"));
        // The injected '#' must also be escaped.
        assert!(result.contains("\\#text"));
    }

    #[test]
    fn test_table_cell_bracket_injection_blocked() {
        let md = "| A | B |\n|---|---|\n| ] #evil[ | x |";
        let result = markdown_to_typst(md);
        assert!(result.contains("\\]"));
        assert!(result.contains("\\#evil"));
    }

    #[test]
    fn test_heading_typst_injection_blocked() {
        // A '#' in heading text must be escaped so it cannot trigger Typst
        // function calls.
        let result = markdown_to_typst("# #set page(width: 10000cm)");
        assert!(result.contains("\\#set"));
        assert!(!result.contains("= #set page"));
    }

    #[test]
    fn test_heading_dollar_injection_blocked() {
        let result = markdown_to_typst("# price $x$");
        assert!(result.contains("\\$x\\$"));
    }

    #[test]
    fn test_code_block_with_embedded_triple_backticks() {
        // The inner ``` must not terminate the outer raw block — fence length
        // should grow to one more than the longest inner backtick run.
        let md = "````\nlet s = \"```\";\n````";
        let result = markdown_to_typst(md);
        // Outer fence must be at least four backticks.
        assert!(result.contains("````"));
        assert!(result.contains("let s = \"```\";"));
    }

    #[test]
    fn test_inline_code_plain_stays_single_backtick() {
        // Typst only interprets a language tag for raw with 3+ backticks, so
        // simple inline code must stay single-backtick: otherwise `let x = 5`
        // would be rendered with "let" stripped as a language tag.
        let result = markdown_to_typst("use `let x = 5` here");
        assert!(result.contains("`let x = 5`"));
        assert!(!result.contains("```let"));
    }

    #[test]
    fn test_inline_code_with_backticks() {
        // Inline code containing a backtick must use a longer fence and pad so
        // the boundary backtick isn't consumed by the fence.
        let result = markdown_to_typst("use `` ` `` as a quote");
        assert!(result.contains("`` ` ``"));
    }

    #[test]
    fn test_link_url_quote_is_escaped() {
        let result = markdown_to_typst("[x](https://e.com/\"a)");
        assert!(result.contains("\\\""));
    }

    /// Creates a directory holding placeholder files. Resolution only checks
    /// that the path is a file with a known extension — decoding is Typst's
    /// job — so the contents are irrelevant here.
    fn fixture_dir(name: &str, files: &[&str]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mdpdf-convert-{}", name));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create fixture dir");
        for file in files {
            std::fs::write(dir.join(file), b"placeholder").expect("write fixture");
        }
        dir
    }

    fn convert_in(dir: &Path, markdown: &str) -> (String, AssetRegistry) {
        let mut registry = AssetRegistry::new();
        let markup = markdown_to_typst_with_assets(markdown, Some(dir), &mut registry);
        (markup, registry)
    }

    #[test]
    fn test_image_resolves_to_virtual_path() {
        let dir = fixture_dir("resolves", &["logo.png"]);
        let (result, registry) = convert_in(&dir, "![ACME logo](logo.png)");

        assert!(result.contains("#image(\"/assets/0.png\", alt: \"ACME logo\")"));
        assert_eq!(registry.assets().len(), 1);
        assert_eq!(registry.assets()[0].real_path, dir.join("logo.png"));
        assert!(registry.warnings().is_empty());
    }

    #[test]
    fn test_image_alt_text_is_not_also_emitted_as_body_text() {
        // Alt text belongs in the alt: attribute; leaking it into the document
        // would print the caption next to the image.
        let dir = fixture_dir("alt-once", &["logo.png"]);
        let (result, _) = convert_in(&dir, "![only once](logo.png)");
        assert_eq!(result.matches("only once").count(), 1);
    }

    #[test]
    fn test_repeated_image_registers_once() {
        let dir = fixture_dir("dedup", &["logo.png"]);
        let (result, registry) = convert_in(&dir, "![a](logo.png)\n\n![b](logo.png)");
        assert_eq!(registry.assets().len(), 1);
        assert_eq!(result.matches("/assets/0.png").count(), 2);
    }

    #[test]
    fn test_remote_image_is_refused_not_fetched() {
        let dir = fixture_dir("remote", &[]);
        let (result, registry) = convert_in(&dir, "![banner](https://example.com/x.png)");

        assert!(registry.assets().is_empty());
        assert!(registry.warnings()[0].contains("remote images are not fetched"));
        // Falls back to the alt text so the document still reads sensibly.
        assert!(result.contains("banner"));
        assert!(!result.contains("#image("));
    }

    #[test]
    fn test_missing_image_warns_and_falls_back_to_alt() {
        let dir = fixture_dir("missing", &[]);
        let (result, registry) = convert_in(&dir, "![the chart](nope.png)");

        assert!(registry.assets().is_empty());
        assert!(registry.warnings()[0].contains("image not found"));
        assert!(result.contains("the chart"));
    }

    #[test]
    fn test_unsupported_image_format_warns() {
        let dir = fixture_dir("unsupported", &["diagram.bmp"]);
        let (_, registry) = convert_in(&dir, "![d](diagram.bmp)");
        assert!(registry.warnings()[0].contains("unsupported image format 'bmp'"));
    }

    #[test]
    fn test_image_alt_text_cannot_inject_typst() {
        // Alt text reaches a Typst string literal, so a quote must be escaped
        // rather than closing the argument.
        let dir = fixture_dir("alt-inject", &["logo.png"]);
        let (result, _) = convert_in(&dir, "![a\") + evil(\"](logo.png)");
        assert!(result.contains("\\\""));
        assert!(!result.contains("\") + evil(\""));
    }

    #[test]
    fn test_unresolved_image_alt_text_is_escaped() {
        // The fallback path writes alt text as body content, so it needs the
        // same escaping as any other text.
        let dir = fixture_dir("alt-escape", &[]);
        let (result, _) = convert_in(&dir, "![#set page(width: 900cm)](nope.png)");
        assert!(result.contains("\\#set"));
    }
}
