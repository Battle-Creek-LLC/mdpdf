# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/Battle-Creek-LLC/mdpdf/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Battle-Creek-LLC/mdpdf/releases/tag/v0.1.0
