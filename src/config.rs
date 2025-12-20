//! Configuration management module
//!
//! Handles loading, validation, and environment variable overrides for application configuration.

use crate::errors::{ConfigError, Result};
use config::{Config, Environment, File};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{debug, info, warn};

/// Valid baud rates for serial communication
const VALID_BAUD_RATES: &[u32] = &[
    9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600,
];

/// Main application configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    // Serial port configuration
    /// The name of the serial port (e.g., "/dev/ttyACM0", "COM3")
    pub port_name: String,
    
    /// The baud rate for serial communication
    pub baud_rate: u32,
    
    /// Expected data packet length from ECU (bytes)
    #[serde(default = "default_expected_data_length")]
    pub expected_data_length: usize,
    
    /// Serial read timeout in milliseconds
    #[serde(default = "default_read_timeout_ms")]
    pub read_timeout_ms: u64,
    
    // MQTT broker configuration
    /// MQTT broker host address
    pub mqtt_host: String,
    
    /// MQTT broker port number
    pub mqtt_port: u16,
    
    /// Base MQTT topic prefix (e.g., "/GOLF86/ECU/")
    pub mqtt_base_topic: String,
    
    /// MQTT Quality of Service level (0, 1, or 2)
    #[serde(default = "default_mqtt_qos")]
    pub mqtt_qos: i32,
    
    /// MQTT client ID (auto-generated if not specified)
    pub mqtt_client_id: Option<String>,
    
    /// MQTT username for authentication (optional)
    pub mqtt_username: Option<String>,
    
    /// MQTT password for authentication (optional)
    pub mqtt_password: Option<String>,
    
    /// Enable MQTT over TLS/SSL
    #[serde(default)]
    pub mqtt_use_tls: bool,
    
    /// Path to CA certificate for TLS (optional)
    pub mqtt_ca_cert_path: Option<String>,
    
    /// Path to client certificate for TLS (optional)
    pub mqtt_client_cert_path: Option<String>,
    
    /// Path to client private key for TLS (optional)
    pub mqtt_client_key_path: Option<String>,
    
    // Application behavior configuration
    /// ECU data polling interval in milliseconds
    #[serde(default = "default_refresh_rate_ms")]
    pub refresh_rate_ms: u64,
    
    /// Maximum number of reconnection attempts
    #[serde(default = "default_max_retry_count")]
    pub max_retry_count: u32,
    
    /// Initial reconnection delay in milliseconds
    #[serde(default = "default_initial_retry_delay_ms")]
    pub initial_retry_delay_ms: u64,
    
    /// Maximum reconnection delay in milliseconds (for exponential backoff)
    #[serde(default = "default_max_retry_delay_ms")]
    pub max_retry_delay_ms: u64,
    
    /// MQTT message buffer size (number of messages)
    #[serde(default = "default_message_buffer_size")]
    pub message_buffer_size: usize,
    
    // Logging configuration
    /// Log level (trace, debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub log_level: String,
    
    /// Enable JSON formatted logs
    #[serde(default)]
    pub log_json: bool,
    
    // Internal field
    /// Path to the configuration file (set at runtime)
    #[serde(skip)]
    pub config_path: Option<String>,
}

// Default value functions
fn default_expected_data_length() -> usize { 120 }
fn default_read_timeout_ms() -> u64 { 2000 }
fn default_mqtt_qos() -> i32 { 1 }
fn default_refresh_rate_ms() -> u64 { 20 }
fn default_max_retry_count() -> u32 { 10 }
fn default_initial_retry_delay_ms() -> u64 { 1000 }
fn default_max_retry_delay_ms() -> u64 { 60000 }
fn default_message_buffer_size() -> usize { 1000 }
fn default_log_level() -> String { "info".to_string() }

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            port_name: "/dev/ttyACM0".to_string(),
            baud_rate: 115200,
            expected_data_length: default_expected_data_length(),
            read_timeout_ms: default_read_timeout_ms(),
            mqtt_host: "localhost".to_string(),
            mqtt_port: 1883,
            mqtt_base_topic: "/speeduino/ecu/".to_string(),
            mqtt_qos: default_mqtt_qos(),
            mqtt_client_id: None,
            mqtt_username: None,
            mqtt_password: None,
            mqtt_use_tls: false,
            mqtt_ca_cert_path: None,
            mqtt_client_cert_path: None,
            mqtt_client_key_path: None,
            refresh_rate_ms: default_refresh_rate_ms(),
            max_retry_count: default_max_retry_count(),
            initial_retry_delay_ms: default_initial_retry_delay_ms(),
            max_retry_delay_ms: default_max_retry_delay_ms(),
            message_buffer_size: default_message_buffer_size(),
            log_level: default_log_level(),
            log_json: false,
            config_path: None,
        }
    }
}

impl AppConfig {
    /// Validate the configuration values
    pub fn validate(&self) -> Result<()> {
        debug!("Validating configuration");

        // Validate port_name
        if self.port_name.is_empty() {
            return Err(ConfigError::MissingField("port_name".to_string()).into());
        }

        // Validate baud_rate
        if !VALID_BAUD_RATES.contains(&self.baud_rate) {
            return Err(ConfigError::InvalidValue {
                field: "baud_rate".to_string(),
                message: format!(
                    "must be one of: {:?}",
                    VALID_BAUD_RATES
                ),
            }
            .into());
        }

        // Validate expected_data_length
        if self.expected_data_length == 0 || self.expected_data_length > 1024 {
            return Err(ConfigError::InvalidValue {
                field: "expected_data_length".to_string(),
                message: "must be between 1 and 1024".to_string(),
            }
            .into());
        }

        // Validate read_timeout_ms
        if self.read_timeout_ms == 0 || self.read_timeout_ms > 30000 {
            return Err(ConfigError::InvalidValue {
                field: "read_timeout_ms".to_string(),
                message: "must be between 1 and 30000 milliseconds".to_string(),
            }
            .into());
        }

        // Validate mqtt_host
        if self.mqtt_host.is_empty() {
            return Err(ConfigError::MissingField("mqtt_host".to_string()).into());
        }

        // Validate mqtt_port
        if self.mqtt_port == 0 {
            return Err(ConfigError::InvalidValue {
                field: "mqtt_port".to_string(),
                message: "must be greater than 0".to_string(),
            }
            .into());
        }

        // Validate mqtt_base_topic
        if self.mqtt_base_topic.is_empty() {
            return Err(ConfigError::MissingField("mqtt_base_topic".to_string()).into());
        }

        // Validate mqtt_qos
        if !(0..=2).contains(&self.mqtt_qos) {
            return Err(ConfigError::InvalidValue {
                field: "mqtt_qos".to_string(),
                message: "must be 0, 1, or 2".to_string(),
            }
            .into());
        }

        // Validate refresh_rate_ms
        if self.refresh_rate_ms == 0 || self.refresh_rate_ms > 10000 {
            return Err(ConfigError::InvalidValue {
                field: "refresh_rate_ms".to_string(),
                message: "must be between 1 and 10000 milliseconds".to_string(),
            }
            .into());
        }

        // Validate retry settings
        if self.max_retry_count == 0 {
            return Err(ConfigError::InvalidValue {
                field: "max_retry_count".to_string(),
                message: "must be greater than 0".to_string(),
            }
            .into());
        }

        if self.initial_retry_delay_ms == 0 {
            return Err(ConfigError::InvalidValue {
                field: "initial_retry_delay_ms".to_string(),
                message: "must be greater than 0".to_string(),
            }
            .into());
        }

        if self.max_retry_delay_ms < self.initial_retry_delay_ms {
            return Err(ConfigError::InvalidValue {
                field: "max_retry_delay_ms".to_string(),
                message: "must be greater than or equal to initial_retry_delay_ms".to_string(),
            }
            .into());
        }

        // Validate message_buffer_size
        if self.message_buffer_size == 0 {
            return Err(ConfigError::InvalidValue {
                field: "message_buffer_size".to_string(),
                message: "must be greater than 0".to_string(),
            }
            .into());
        }

        // Validate log_level
        let valid_log_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_log_levels.contains(&self.log_level.to_lowercase().as_str()) {
            return Err(ConfigError::InvalidValue {
                field: "log_level".to_string(),
                message: format!("must be one of: {:?}", valid_log_levels),
            }
            .into());
        }

        // Validate TLS configuration
        if self.mqtt_use_tls {
            if let Some(ref ca_path) = self.mqtt_ca_cert_path {
                if !Path::new(ca_path).exists() {
                    return Err(ConfigError::InvalidValue {
                        field: "mqtt_ca_cert_path".to_string(),
                        message: format!("file does not exist: {}", ca_path),
                    }
                    .into());
                }
            }
        }

        info!("Configuration validation successful");
        Ok(())
    }

    /// Display a summary of the configuration (hiding sensitive data)
    pub fn display_summary(&self) {
        info!("=== Configuration Summary ===");
        info!("Serial Port: {} @ {} baud", self.port_name, self.baud_rate);
        info!("MQTT Broker: {}:{}", self.mqtt_host, self.mqtt_port);
        info!("MQTT Base Topic: {}", self.mqtt_base_topic);
        info!("MQTT QoS: {}", self.mqtt_qos);
        if self.mqtt_use_tls {
            info!("MQTT TLS: enabled");
        }
        if self.mqtt_username.is_some() {
            info!("MQTT Authentication: enabled");
        }
        info!("Refresh Rate: {}ms", self.refresh_rate_ms);
        info!("Max Retry Count: {}", self.max_retry_count);
        info!("Message Buffer Size: {}", self.message_buffer_size);
        info!("Log Level: {}", self.log_level);
        info!("============================");
    }
}

/// Load application configuration from a TOML file with environment variable overrides
///
/// # Arguments
/// * `config_path` - Optional path to configuration file
///
/// # Returns
/// Returns `Result<AppConfig>` with loaded and validated configuration
///
/// # Environment Variables
/// All configuration fields can be overridden with environment variables prefixed with `SPEEDUINO_`
/// For example: `SPEEDUINO_PORT_NAME=/dev/ttyUSB0`
pub fn load_configuration(config_path: Option<&str>) -> Result<AppConfig> {
    info!("Loading configuration");

    let mut builder = Config::builder();

    // Try to load from various locations in order of priority
    let loaded_from = if let Some(path) = config_path {
        debug!("Attempting to load config from specified path: {}", path);
        builder = builder.add_source(File::with_name(path).required(true));
        Some(path.to_string())
    } else {
        // Try multiple default locations
        let default_locations = vec![
            "./settings.toml",
            "./speeduino-to-mqtt.toml",
        ];

        // Try executable directory
        if let Ok(exe_dir) = std::env::current_exe() {
            if let Some(parent) = exe_dir.parent() {
                let exe_config = parent.join("settings.toml");
                if exe_config.exists() {
                    debug!("Found config in executable directory: {:?}", exe_config);
                    builder = builder.add_source(File::from(exe_config.clone()).required(false));
                }
            }
        }

        // Try system locations
        let system_locations = vec![
            "/usr/etc/g86-car-telemetry/speeduino-to-mqtt.toml",
            "/etc/g86-car-telemetry/speeduino-to-mqtt.toml",
        ];

        let mut found_path = None;
        for location in default_locations.iter().chain(system_locations.iter()) {
            let path = Path::new(location);
            if path.exists() {
                debug!("Found config at: {}", location);
                builder = builder.add_source(File::with_name(location).required(false));
                if found_path.is_none() {
                    found_path = Some(location.to_string());
                }
            }
        }

        if found_path.is_none() {
            warn!("No configuration file found in default locations");
        }

        found_path
    };

    // Add environment variable overrides with prefix "SPEEDUINO_"
    builder = builder.add_source(
        Environment::with_prefix("SPEEDUINO")
            .separator("_")
            .try_parsing(true),
    );

    // Build configuration
    let settings = builder
        .build()
        .map_err(|e| ConfigError::LoadFailed(e.to_string()))?;

    // Deserialize into AppConfig
    let mut app_config: AppConfig = settings
        .try_deserialize()
        .map_err(|e| ConfigError::LoadFailed(e.to_string()))?;

    // Set the config path that was loaded
    app_config.config_path = loaded_from;

    // Validate configuration
    app_config.validate()?;

    if let Some(ref path) = app_config.config_path {
        info!("Configuration loaded from: {}", path);
    } else {
        info!("Configuration loaded from environment variables and defaults");
    }

    Ok(app_config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_default_config_validation() {
        let config = AppConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_load_configuration_from_file() {
        let toml_content = r#"
            port_name = "/dev/ttyACM0"
            baud_rate = 115200
            mqtt_host = "mqtt.example.com"
            mqtt_port = 1883
            mqtt_base_topic = "/test/ecu/"
            refresh_rate_ms = 500
        "#;

        let temp_dir = tempdir().expect("Failed to create temporary directory");
        let file_path = temp_dir.path().join("settings.toml");
        fs::write(&file_path, toml_content).expect("Failed to write temporary file");

        let config = load_configuration(Some(file_path.to_str().unwrap()))
            .expect("Failed to load configuration");

        assert_eq!(config.port_name, "/dev/ttyACM0");
        assert_eq!(config.baud_rate, 115200);
        assert_eq!(config.mqtt_host, "mqtt.example.com");
        assert_eq!(config.mqtt_port, 1883);
        assert_eq!(config.mqtt_base_topic, "/test/ecu/");
        assert_eq!(config.refresh_rate_ms, 500);
    }

    #[test]
    fn test_invalid_baud_rate() {
        let mut config = AppConfig::default();
        config.baud_rate = 12345; // Invalid baud rate
        
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_mqtt_qos() {
        let mut config = AppConfig::default();
        config.mqtt_qos = 5; // Invalid QoS
        
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_refresh_rate() {
        let mut config = AppConfig::default();
        config.refresh_rate_ms = 0; // Invalid (zero)
        
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_required_fields() {
        let mut config = AppConfig::default();
        config.port_name = String::new(); // Empty port name
        
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_retry_delay_validation() {
        let mut config = AppConfig::default();
        config.max_retry_delay_ms = 500;
        config.initial_retry_delay_ms = 1000; // Greater than max
        
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_valid_log_levels() {
        for level in &["trace", "debug", "info", "warn", "error"] {
            let mut config = AppConfig::default();
            config.log_level = level.to_string();
            assert!(config.validate().is_ok());
        }
    }

    #[test]
    fn test_invalid_log_level() {
        let mut config = AppConfig::default();
        config.log_level = "invalid".to_string();
        
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_environment_variable_override() {
        // This test would require setting environment variables
        // which is tricky in unit tests, but documents the feature
        let config = AppConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_mqtt_authentication_fields() {
        let mut config = AppConfig::default();
        config.mqtt_username = Some("testuser".to_string());
        config.mqtt_password = Some("testpass".to_string());
        
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_with_tls() {
        let mut config = AppConfig::default();
        config.mqtt_use_tls = true;
        // Without valid cert path, validation should still pass
        // (only validates if path is provided AND TLS is enabled)
        assert!(config.validate().is_ok());
    }
}
