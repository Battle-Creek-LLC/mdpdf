# mdpdf

Convert Markdown to beautifully typeset PDFs, powered by [Typst](https://typst.app/).

`mdpdf` parses GitHub-flavored Markdown, translates it into Typst markup, and compiles it to a PDF — no LaTeX, no headless browser, no network access. It's a single self-contained Rust binary.

## Features

- GitHub-flavored Markdown: tables, strikethrough, task lists, fenced code blocks, block quotes
- Clean, print-ready typography with configurable font size, page size, and margins
- Render multiple files at once — one PDF per input, or `--combine` them into a single document
- Read from stdin with `-`
- `--open` to launch the rendered PDF in your system viewer

## Install

From source (requires a recent Rust toolchain):

```sh
git clone https://github.com/jstockdi/mdpdf.git
cd mdpdf
cargo install --path .
```

This installs the `mdpdf` binary into `~/.cargo/bin`.

## Usage

```sh
# Single file → notes.pdf
mdpdf notes.md

# Custom output path
mdpdf notes.md -o handout.pdf

# Multiple files → one PDF each
mdpdf chapter1.md chapter2.md chapter3.md

# Combine multiple files into a single PDF (page break between each)
mdpdf --combine chapter1.md chapter2.md chapter3.md -o book.pdf

# Read from stdin
cat notes.md | mdpdf - -o notes.pdf

# Tweak typography and open the result
mdpdf notes.md --font-size 11 --margin 20 --page-size a4 --open
```

### Options

| Flag | Description |
| --- | --- |
| `-o, --output <PATH>` | Output PDF path. Defaults to `<input>.pdf`, or `combined.pdf` with `--combine`. |
| `-c, --combine` | Merge all inputs into one PDF, separated by page breaks. |
| `-t, --title <TITLE>` | PDF document title metadata. |
| `--font-size <PT>` | Base body font size in points (default: `10`). |
| `--page-size <SIZE>` | `letter` or `a4` (default: `letter`). |
| `--margin <MM>` | Page margins in millimeters (default: `25`). |
| `--open` | Open each generated PDF in the system's default viewer. |

## License

MIT — see [LICENSE](LICENSE).
