# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] — 2026-08-14

### Added

- Image embedding: `![alt](path.png)` renders PNG, JPEG, GIF, SVG, and WebP,
  with paths resolved relative to the Markdown file. Images wider than the text
  block scale down to fit.
- Header and footer bands, each with `left`, `center`, and `right` zones holding
  Markdown, configured in TOML rather than on the command line.
- Header/footer tokens `{page}`, `{pages}`, `{title}`, and `{date}`, substituted
  only inside zones so body text containing `{page}` is unaffected.
- Configuration files: `~/.config/mdpdf/config.toml`, the nearest ancestor
  `.mdpdf.toml`, and `--config <PATH>`, merged per field in that order with CLI
  flags winning.
- `--no-config` to skip config discovery for reproducible output.

### Changed

- `--font-size`, `--page-size`, and `--margin` no longer carry clap defaults, so
  an explicitly passed value can be distinguished from an unset one when merging
  with config files. Defaults are unchanged when no config is present.

### Security

- Remote image URLs are refused rather than fetched, preserving the property
  that `mdpdf` makes no network requests.
- Typst can only read images the converter already resolved: assets are served
  from an in-memory map under generated virtual paths, so no user-controlled
  path string reaches the generated markup.

## [0.1.0] — 2026-05-20

First tagged release. `mdpdf` converts GitHub-flavored Markdown to PDF
via Typst, with no LaTeX toolchain, headless browser, or network access.

### Added

- Single-file conversion: `mdpdf notes.md` writes `notes.pdf`.
- Multi-input support: pass several Markdown files to render one PDF per
  input, or `--combine` to merge them into a single document separated by
  page breaks.
- `-` reads Markdown from stdin.
- `--open` launches the rendered PDF in the system's default viewer.
- Typography knobs: `--font-size`, `--page-size` (`letter`/`a4`), and
  `--margin` (millimeters).
- `-t/--title` sets the PDF document-title metadata.
- GitHub-flavored Markdown features: tables, strikethrough, task lists,
  fenced code blocks, and block quotes.
- Hardened Typst escaping so arbitrary Markdown input cannot break out of
  generated Typst markup.
- Prebuilt binaries on each tagged release for Linux (x86_64, aarch64),
  macOS (x86_64, aarch64), and Windows (x86_64).

[Unreleased]: https://github.com/Battle-Creek-LLC/mdpdf/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/Battle-Creek-LLC/mdpdf/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Battle-Creek-LLC/mdpdf/releases/tag/v0.1.0
