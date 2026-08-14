//! Configuration files for header/footer zones and page defaults.
//!
//! Three layers compose, lowest precedence first: the user config directory,
//! the nearest ancestor `.mdpdf.toml`, then an explicit `--config` path. CLI
//! flags override all of them, which `main` applies after `merge`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

pub const PROJECT_CONFIG_NAME: &str = ".mdpdf.toml";

/// The default height applied to images inside a header or footer band, in mm.
/// Without a bound, a large logo would expand the band across the page.
pub const DEFAULT_BAND_IMAGE_HEIGHT_MM: f64 = 8.0;
pub const DEFAULT_BAND_FONT_SIZE_PT: f64 = 8.0;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigFile {
    #[serde(default)]
    pub header: BandFile,
    #[serde(default)]
    pub footer: BandFile,
    #[serde(default)]
    pub page: PageFile,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BandFile {
    pub left: Option<String>,
    pub center: Option<String>,
    pub right: Option<String>,
    pub image_height: Option<f64>,
    pub font_size: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageFile {
    pub size: Option<String>,
    pub margin: Option<f64>,
    pub font_size: Option<f64>,
    pub title: Option<String>,
}

/// One zone's Markdown, paired with the directory of the config file that set
/// it. Relative image paths resolve against that directory, so a logo in the
/// user config keeps working when a project config overrides a different zone.
#[derive(Debug, Clone)]
pub struct Zone {
    pub markdown: String,
    pub base: PathBuf,
}

#[derive(Debug, Default, Clone)]
pub struct Band {
    pub left: Option<Zone>,
    pub center: Option<Zone>,
    pub right: Option<Zone>,
    pub image_height_mm: Option<f64>,
    pub font_size_pt: Option<f64>,
}

impl Band {
    pub fn is_empty(&self) -> bool {
        self.left.is_none() && self.center.is_none() && self.right.is_none()
    }

    fn overlay(&mut self, file: BandFile, dir: &Path) {
        let zone = |value: Option<String>| {
            value.map(|markdown| Zone {
                markdown,
                base: dir.to_path_buf(),
            })
        };
        if let Some(z) = zone(file.left) {
            self.left = Some(z);
        }
        if let Some(z) = zone(file.center) {
            self.center = Some(z);
        }
        if let Some(z) = zone(file.right) {
            self.right = Some(z);
        }
        if let Some(v) = file.image_height {
            self.image_height_mm = Some(v);
        }
        if let Some(v) = file.font_size {
            self.font_size_pt = Some(v);
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Config {
    pub header: Band,
    pub footer: Band,
    pub page_size: Option<String>,
    pub margin_mm: Option<f64>,
    pub font_size_pt: Option<f64>,
    pub title: Option<String>,
}

impl Config {
    /// Applies one config file on top of this one. Later calls win field by
    /// field, so a project config that sets only `[footer]` keeps the user
    /// config's header.
    fn overlay(&mut self, file: ConfigFile, dir: &Path) {
        self.header.overlay(file.header, dir);
        self.footer.overlay(file.footer, dir);
        if let Some(v) = file.page.size {
            self.page_size = Some(v);
        }
        if let Some(v) = file.page.margin {
            self.margin_mm = Some(v);
        }
        if let Some(v) = file.page.font_size {
            self.font_size_pt = Some(v);
        }
        if let Some(v) = file.page.title {
            self.title = Some(v);
        }
    }
}

/// Builds the merged config for a render.
///
/// `start_dir` is where the upward search for `.mdpdf.toml` begins — the
/// directory of the first input file, or the working directory for stdin.
pub fn load(start_dir: &Path, explicit: Option<&Path>, no_config: bool) -> Result<Config> {
    let mut config = Config::default();
    if no_config {
        // An explicit --config alongside --no-config is still honored; the flag
        // suppresses discovery, not a path the user named outright.
        if let Some(path) = explicit {
            overlay_path(&mut config, path, true)?;
        }
        return Ok(config);
    }

    if let Some(path) = user_config_path() {
        overlay_path(&mut config, &path, false)?;
    }
    if let Some(path) = find_project_config(start_dir) {
        overlay_path(&mut config, &path, false)?;
    }
    if let Some(path) = explicit {
        overlay_path(&mut config, path, true)?;
    }
    Ok(config)
}

/// Reads and merges one config file. A missing file is an error only when the
/// user named it explicitly; a discovered path that vanished is skipped.
fn overlay_path(config: &mut Config, path: &Path, required: bool) -> Result<()> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && !required => return Ok(()),
        Err(e) => {
            return Err(e).with_context(|| format!("Failed to read config {}", path.display()))
        }
    };
    let parsed: ConfigFile = toml::from_str(&text)
        .with_context(|| format!("Failed to parse config {}", path.display()))?;
    let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    config.overlay(parsed, &dir);
    Ok(())
}

/// `$XDG_CONFIG_HOME/mdpdf/config.toml`, falling back to `~/.config` on unix
/// and `%APPDATA%` on Windows.
pub fn user_config_path() -> Option<PathBuf> {
    if cfg!(windows) {
        return env_dir("APPDATA").map(|d| d.join("mdpdf").join("config.toml"));
    }
    let base = env_dir("XDG_CONFIG_HOME").or_else(|| env_dir("HOME").map(|h| h.join(".config")))?;
    Some(base.join("mdpdf").join("config.toml"))
}

fn env_dir(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

/// Walks up from `start` looking for `.mdpdf.toml`, so a config at a repo root
/// applies to documents in its subdirectories.
///
/// `start` is canonicalized first: the parent of a relative `"."` is `""`, which
/// would end the walk immediately and hide a config one directory up.
pub fn find_project_config(start: &Path) -> Option<PathBuf> {
    let absolute = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());
    let mut dir = Some(absolute.as_path());
    while let Some(current) = dir {
        let candidate = current.join(PROJECT_CONFIG_NAME);
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = current.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> ConfigFile {
        toml::from_str(text).expect("valid config")
    }

    #[test]
    fn project_config_overrides_user_config_per_field() {
        let mut config = Config::default();
        config.overlay(
            parse("[header]\nleft = \"user-left\"\ncenter = \"user-center\"\n"),
            Path::new("/user"),
        );
        config.overlay(parse("[header]\ncenter = \"project-center\"\n"), Path::new("/proj"));

        // The overridden zone takes the project value, the untouched zone keeps
        // the user value *and* the user directory it must resolve images against.
        assert_eq!(config.header.center.as_ref().unwrap().markdown, "project-center");
        assert_eq!(config.header.center.as_ref().unwrap().base, Path::new("/proj"));
        assert_eq!(config.header.left.as_ref().unwrap().markdown, "user-left");
        assert_eq!(config.header.left.as_ref().unwrap().base, Path::new("/user"));
    }

    #[test]
    fn page_defaults_merge_independently() {
        let mut config = Config::default();
        config.overlay(parse("[page]\nmargin = 20.0\nsize = \"a4\"\n"), Path::new("/user"));
        config.overlay(parse("[page]\nmargin = 15.0\n"), Path::new("/proj"));
        assert_eq!(config.margin_mm, Some(15.0));
        assert_eq!(config.page_size.as_deref(), Some("a4"));
    }

    #[test]
    fn unknown_keys_are_rejected() {
        // A typo should surface as an error rather than being silently ignored.
        let result: Result<ConfigFile, _> = toml::from_str("[header]\nlfet = \"oops\"\n");
        assert!(result.is_err());
    }

    #[test]
    fn empty_config_yields_no_bands() {
        let config = Config::default();
        assert!(config.header.is_empty());
        assert!(config.footer.is_empty());
    }
}
