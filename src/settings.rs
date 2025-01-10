use std::fs;

use anyhow::Context;
use serde::{self, Deserialize, Serialize};

const SETTINGS_PATH: &str = "Settings.toml";

/// Holds the settings for the application converted from a TOML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Settings {
    /// Link to webpage with links to ePUB files
    /// Don't change this unless it stops working!
    pub url: String,

    /// The number of issues to download, starting with the most recent issue
    pub limit: usize,
}

impl Settings {
    pub fn load() -> anyhow::Result<Self> {
        let s = fs::read_to_string(SETTINGS_PATH)
            .with_context(|| format!("can't read file {}", SETTINGS_PATH))?;

        toml::from_str(&s)
            .with_context(|| format!("can't parse TOML content from {}", SETTINGS_PATH))
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            url: String::from("https://subscribe.pcspublink.com/websis/DigitalIssues/TBLT/2330"),
            limit: 1,
        }
    }
}
