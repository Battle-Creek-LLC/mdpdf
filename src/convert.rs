use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

pub fn markdown_to_typst(markdown: &str) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_GFM;
    let parser = Parser::new_ext(markdown, options);
    let mut converter = Converter::new();
    converter.convert(parser);
    converter.output
}

struct Converter {
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
    in_heading: bool,
    needs_paragraph_break: bool,
    inline_stack: Vec<InlineStyle>,
    in_block_quote: bool,
    block_quote_buf: String,
    in_block_quote_depth: usize,
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

impl Converter {
    fn new() -> Self {
        Self {
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
            in_heading: false,
            needs_paragraph_break: false,
            inline_stack: Vec::new(),
            in_block_quote: false,
            block_quote_buf: String::new(),
            in_block_quote_depth: 0,
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
                '*' | '_' | '`' | '#' | '<' | '>' | '@' | '$' | '\\' | '~' => {
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
                self.in_heading = true;
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

                self.output.push_str(&format!(
                    "#table(\n  columns: {},\n  align: ({}),\n  stroke: 0.5pt + luma(200),\n  inset: 8pt,\n",
                    cols,
                    align_strs.join(", "),
                ));
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
                self.in_heading = false;
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

                if self.in_block_quote {
                    self.block_quote_buf.push_str(&format!(
                        "```{}\n{}\n```\n",
                        lang_tag, code
                    ));
                } else {
                    self.output.push_str(&format!(
                        "```{}\n{}\n```\n",
                        lang_tag, code
                    ));
                }
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
            TagEnd::Table => {
                self.output.push_str(")\n");
                self.in_table = false;
                self.needs_paragraph_break = true;
            }
            TagEnd::TableHead => {
                self.in_table_head = false;
                let cells: Vec<String> = self.table_row_cells
                    .iter()
                    .map(|c| format!("[*{}*]", c.trim()))
                    .collect();
                self.output.push_str(&format!("  table.header({}),\n", cells.join(", ")));
                self.table_row_cells.clear();
            }
            TagEnd::TableRow => {
                if !self.in_table_head {
                    for cell in &self.table_row_cells {
                        self.output.push_str(&format!("  [{}],\n", cell.trim()));
                    }
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
        if self.in_code_block {
            self.code_block_buf.push_str(text);
        } else if self.in_heading {
            self.push(text);
        } else {
            self.push(&Self::escape_typst(text));
        }
    }

    fn inline_code(&mut self, code: &str) {
        if code.contains('`') {
            self.push(&format!("```{}```", code));
        } else {
            self.push(&format!("`{}`", code));
        }
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
        self.replace_last_list_marker(symbol);
    }

    fn replace_last_list_marker(&mut self, replacement: &str) {
        let target = self.active_buf();
        // Find the last "- " and insert the checkbox symbol after it
        if let Some(pos) = target.rfind("- ") {
            let insert_pos = pos + 2;
            target.insert_str(insert_pos, replacement);
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
}
