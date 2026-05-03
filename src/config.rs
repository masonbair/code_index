//! Configuration handling for code-index

use crate::daemon::default_db_path;
use std::path::PathBuf;

/// Application configuration
#[derive(Debug, Clone)]
pub struct Config {
    /// Path to the SQLite database
    pub db_path: PathBuf,
    /// Verbosity level (0 = quiet, 1 = normal, 2+ = verbose)
    pub verbosity: u8,
    /// Output format
    pub output_format: OutputFormat,
}

impl Config {
    pub fn new() -> Self {
        Self {
            db_path: default_db_path(),
            verbosity: 1,
            output_format: OutputFormat::Human,
        }
    }

    pub fn with_db_path(mut self, path: PathBuf) -> Self {
        self.db_path = path;
        self
    }

    pub fn with_verbosity(mut self, verbosity: u8) -> Self {
        self.verbosity = verbosity;
        self
    }

    pub fn with_output_format(mut self, format: OutputFormat) -> Self {
        self.output_format = format;
        self
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

/// Output format for CLI commands
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// Human-readable text output
    #[default]
    Human,
    /// JSON output for machine parsing
    Json,
    /// Compact output (file:line only)
    Compact,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "human" | "text" => Ok(OutputFormat::Human),
            "json" => Ok(OutputFormat::Json),
            "compact" => Ok(OutputFormat::Compact),
            _ => Err(format!("Unknown output format: {}", s)),
        }
    }
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Human => write!(f, "human"),
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Compact => write!(f, "compact"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = Config::new();
        assert_eq!(config.verbosity, 1);
        assert_eq!(config.output_format, OutputFormat::Human);
    }

    #[test]
    fn test_config_builder() {
        let config = Config::new()
            .with_verbosity(2)
            .with_output_format(OutputFormat::Json);

        assert_eq!(config.verbosity, 2);
        assert_eq!(config.output_format, OutputFormat::Json);
    }

    #[test]
    fn test_output_format_parsing() {
        assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
        assert_eq!(
            "human".parse::<OutputFormat>().unwrap(),
            OutputFormat::Human
        );
        assert_eq!(
            "compact".parse::<OutputFormat>().unwrap(),
            OutputFormat::Compact
        );
        assert!("invalid".parse::<OutputFormat>().is_err());
    }
}
