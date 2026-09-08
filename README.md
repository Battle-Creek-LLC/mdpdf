# mdpdf

Convert Markdown to beautifully typeset PDFs, powered by [Typst](https://typst.app/).

`mdpdf` parses GitHub-flavored Markdown, translates it into Typst markup, and compiles it to a PDF — no LaTeX, no headless browser, no network access. It's a single self-contained Rust binary.

## Features

- GitHub-flavored Markdown: tables, strikethrough, task lists, fenced code blocks, block quotes
- Embedded images — PNG, JPEG, GIF, SVG, and WebP
- Headers and footers with page numbers, a logo, and a date, set from a config file
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

# Use an explicit config for headers and footers
mdpdf notes.md --config branding.toml

# Ignore every config file on disk
mdpdf notes.md --no-config
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
| `--config <PATH>` | Config file to use, overriding the user and project configs. |
| `--no-config` | Ignore the user config and any discovered `.mdpdf.toml`. |
| `--open` | Open each generated PDF in the system's default viewer. |

## Images

Standard Markdown image syntax works, with paths resolved relative to the
Markdown file:

```markdown
![Architecture diagram](diagrams/arch.png)
```

PNG, JPEG, GIF, SVG, and WebP are supported. Images wider than the text block
are scaled down to fit.

Remote images are **not** fetched — `mdpdf` makes no network requests. An
`http://` or `https://` source is reported on stderr and falls back to rendering
the alt text, as does a missing file, so a broken reference never fails a build.

## Headers and footers

Headers and footers are configured in TOML, not on the command line. Each band
has three zones — `left`, `center`, and `right` — and each zone holds Markdown:

```toml
[header]
left   = "![](logo.svg)"
center = "**{title}**"
right  = "{date}"

[footer]
center = "{page} of {pages}"
```

### Tokens

| Token | Renders as |
| --- | --- |
| `{page}` | The current page number. |
| `{pages}` | The total page count. |
| `{title}` | The document title, from `-t/--title` or `page.title`. |
| `{date}` | Today's date, as `YYYY-MM-DD`. |

Tokens are only substituted inside header and footer zones — a literal `{page}`
in your document body is left alone.

### Configuration files

Three sources compose, each overriding the one before it:

1. `~/.config/mdpdf/config.toml` — or `$XDG_CONFIG_HOME/mdpdf/config.toml`, and
   `%APPDATA%\mdpdf\config.toml` on Windows
2. the nearest `.mdpdf.toml`, searching upward from the document's directory
3. `--config <PATH>`

Command-line flags override all three. Merging is per field, so a project
`.mdpdf.toml` that sets only `[footer]` keeps the header from your user config.
Image paths resolve relative to the config file that declared them, so a logo in
your user config keeps working in every project.

Use `--no-config` to ignore both discovered configs, for reproducible output in
CI.

### All config keys

```toml
[header]              # and [footer], with the same keys
left         = ""     # Markdown
center       = ""
right        = ""
image_height = 8      # mm; bounds logo height so it can't inflate the band
font_size    = 8      # pt

[page]
size      = "letter"  # letter | a4
margin    = 25        # mm
font_size = 10        # pt
title     = ""        # PDF title metadata; --title wins
```

## License

MIT — see [LICENSE](LICENSE).
