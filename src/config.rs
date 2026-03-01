//! Configuration types and helpers for `pixiv-exporter`.
//!
//! This module defines the JSON-serializable config schema, default values, and
//! utilities to load config files and emit schema/example documents.

use std::{fs::File, path::Path, time::Duration};

use anyhow::{Result, anyhow};
use rand::RngExt;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty as to_json_pretty;
use serde_with::serde_as;

/// Target users and standalone works that should be scraped.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TargetConfig {
    #[serde(default)]
    pub users: Vec<u64>,
    #[serde(default)]
    pub works: Vec<u64>,
}

/// Scrape interval configuration with optional randomized variance.
#[serde_as]
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum IntervalConfig {
    Fixed(#[serde_as(as = "serde_with::DurationSecondsWithFrac<f64>")] Duration),
    WithVariance {
        #[serde_as(as = "serde_with::DurationSecondsWithFrac<f64>")]
        interval: Duration,
        variance: f64,
    },
}

impl IntervalConfig {
    /// The minimum negative variance is -50%
    pub const MIN_NEGATIVE_VARIANCE: f64 = -0.5;
    /// The maximum positive variance is +100%
    pub const MAX_POSITIVE_VARIANCE: f64 = 1.0;

    /// Generates the effective interval for one wait cycle.
    ///
    /// `Fixed` returns the configured duration as-is. `WithVariance` applies a
    /// random multiplier in `[-variance, +variance]`, clamped to supported
    /// bounds to avoid extreme values.
    pub fn gen_interval(&self) -> Duration {
        match self {
            IntervalConfig::Fixed(duration) => *duration,
            IntervalConfig::WithVariance { interval, variance } => {
                let min_variance = (-variance).clamp(Self::MIN_NEGATIVE_VARIANCE, 0.0);
                let max_variance = variance.clamp(0.0, Self::MAX_POSITIVE_VARIANCE);
                let random_variance = rand::rng().random_range(min_variance..=max_variance);
                Duration::from_secs_f64(interval.as_secs_f64() * (1.0 + random_variance))
            }
        }
    }
}

/// Scraping cadence settings for rounds and per-item delays.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ScrapeConfig {
    #[serde(default = "default_scrape_interval")]
    pub scrape_interval: IntervalConfig,
    #[serde(default = "default_independent_item_interval")]
    pub independent_item_interval: IntervalConfig,
    #[serde(default = "default_user_item_interval")]
    pub user_item_interval: IntervalConfig,
}

fn default_scrape_interval() -> IntervalConfig {
    IntervalConfig::WithVariance {
        interval: Duration::from_mins(30),
        variance: 0.2,
    }
}

fn default_independent_item_interval() -> IntervalConfig {
    IntervalConfig::WithVariance {
        interval: Duration::from_secs_f64(1.5),
        variance: 0.1,
    }
}

fn default_user_item_interval() -> IntervalConfig {
    IntervalConfig::WithVariance {
        interval: Duration::from_secs_f64(0.1),
        variance: 0.1,
    }
}

impl Default for ScrapeConfig {
    fn default() -> Self {
        Self {
            scrape_interval: default_scrape_interval(),
            independent_item_interval: default_independent_item_interval(),
            user_item_interval: default_user_item_interval(),
        }
    }
}

/// HTTP server bind address and port settings.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ServerConfig {
    #[serde(default = "default_bind")]
    pub bind: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

/// The default bind is localhost
fn default_bind() -> String {
    "127.0.0.1".to_string()
}

/// The default port is 6825
fn default_port() -> u16 {
    6825
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: default_bind(),
            port: default_port(),
        }
    }
}

/// String value that can be inlined or loaded from an environment variable.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum StringOrEnvRef {
    String(String),
    EnvRef { env: String },
}

impl StringOrEnvRef {
    /// Resolves this value to a concrete string.
    ///
    /// For `String`, it returns the literal value. For `EnvRef`, it reads the
    /// variable named by `env` and returns an error when the variable is absent.
    pub fn get_value(&self) -> Result<String> {
        match self {
            StringOrEnvRef::String(value) => Ok(value.clone()),
            StringOrEnvRef::EnvRef { env } => std::env::var(env)
                .map_err(|e| anyhow!("Environment variable {} not found: {}", env, e)),
        }
    }
}

/// Top-level configuration used by the exporter runtime.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Config {
    pub target: TargetConfig,
    #[serde(default)]
    pub scrape: ScrapeConfig,
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default = "default_refresh_token")]
    pub refresh_token: StringOrEnvRef,
}

fn default_refresh_token() -> StringOrEnvRef {
    StringOrEnvRef::EnvRef {
        env: "PIXIV_REFRESH_TOKEN".to_string(),
    }
}

impl Config {
    /// Returns the pretty-printed JSON schema for this configuration type.
    pub fn json_schema() -> String {
        to_json_pretty(&schemars::schema_for!(Config)).unwrap()
    }

    /// Loads and parses configuration from a JSON file path.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Config> {
        let path = path.as_ref();
        let file = File::open(path)
            .map_err(|e| anyhow!("Failed to open config file: {}: {}", path.display(), e))?;
        let config: Config = serde_json::from_reader(file)
            .map_err(|e| anyhow!("Failed to parse config file: {}: {}", path.display(), e))?;
        Ok(config)
    }

    /// Builds a pretty-printed example configuration document.
    pub fn example_config() -> String {
        let config = Self {
            target: TargetConfig {
                users: vec![],
                works: vec![],
            },
            scrape: Default::default(),
            server: Default::default(),
            refresh_token: default_refresh_token(),
        };

        to_json_pretty(&config).unwrap()
    }
}
