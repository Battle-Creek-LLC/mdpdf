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

Prebuilt binaries are attached to each [release](https://github.com/Battle-Creek-LLC/mdpdf/releases).
Each archive ships a single `mdpdf` (or `mdpdf.exe`) binary plus a matching
`.sha256` checksum.

### macOS (Apple Silicon)

```sh
gh release download --pattern 'mdpdf-aarch64-apple-darwin.tar.gz' -R Battle-Creek-LLC/mdpdf
tar -xzf mdpdf-aarch64-apple-darwin.tar.gz
sudo mv mdpdf /usr/local/bin/
```

### macOS (Intel)

```sh
gh release download --pattern 'mdpdf-x86_64-apple-darwin.tar.gz' -R Battle-Creek-LLC/mdpdf
tar -xzf mdpdf-x86_64-apple-darwin.tar.gz
sudo mv mdpdf /usr/local/bin/
```

> Binaries aren't notarized. On first run macOS may block them — clear the
> quarantine attribute with `xattr -d com.apple.quarantine /usr/local/bin/mdpdf`.

### Linux (x86_64)

```sh
gh release download --pattern 'mdpdf-x86_64-unknown-linux-gnu.tar.gz' -R Battle-Creek-LLC/mdpdf
tar -xzf mdpdf-x86_64-unknown-linux-gnu.tar.gz
sudo mv mdpdf /usr/local/bin/
```

### Linux (ARM64)

```sh
gh release download --pattern 'mdpdf-aarch64-unknown-linux-gnu.tar.gz' -R Battle-Creek-LLC/mdpdf
tar -xzf mdpdf-aarch64-unknown-linux-gnu.tar.gz
sudo mv mdpdf /usr/local/bin/
```

### Windows (PowerShell)

```powershell
gh release download --pattern 'mdpdf-x86_64-pc-windows-msvc.zip' -R Battle-Creek-LLC/mdpdf
Expand-Archive mdpdf-x86_64-pc-windows-msvc.zip -DestinationPath .
# Move mdpdf.exe into a directory on your PATH, e.g.:
Move-Item mdpdf.exe "$env:USERPROFILE\bin\"
```

### From source

```sh
cargo install --git https://github.com/Battle-Creek-LLC/mdpdf
```

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
