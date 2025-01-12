use config::{Config, File};
use std::path::Path;

/// Struct to hold the application configuration.
#[derive(Clone)]
pub struct AppConfig {
    /// The name of the serial port.
    pub port_name: String,

    /// The baud rate for the serial port.
    pub baud_rate: i64,

    /// The MQTT broker host address.
    pub mqtt_host: String,

    /// The MQTT broker port number.
    pub mqtt_port: i64,

    /// The base topic of MQTT where data is pushed.
    pub mqtt_base_topic: String,

    /// Refresh rate in milliseconds.
    pub refresh_rate_ms: Option<u64>,

    // Optional: Path to the configuration file
    pub config_path: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            port_name: String::new(),
            baud_rate: 9600, // Provide a default baud rate value
            mqtt_host: String::new(),
            mqtt_port: 1883, // Provide a default MQTT port value
            mqtt_base_topic: String::new(),
            refresh_rate_ms: Some(1000), // Set the default refresh rate to 1000ms
            config_path: None
        }
    }
}

/// Load application configuration from a TOML file.
///
/// This function reads the configuration settings from a TOML file.
///
/// # Arguments
/// - `config_path`: An optional path to the configuration file.
///
/// # Returns
/// Returns a `Result` containing either the `AppConfig` struct with the loaded configuration or an error message.
pub fn load_configuration(config_path: Option<&str>) -> Result<AppConfig, String> {
    // Create a default configuration
    let mut settings = Config::default();

    // Helper function to load configuration from a given path
    fn load_from_path(path: &str, settings: &mut Config) -> Result<(), String> {
        Config::builder()
            .add_source(File::with_name(path))
            .build()
            .map(|config| *settings = config)
            .map_err(|err| format!("{}", err))
    }

    // Try to load from the passed config_path
    if let Some(path) = config_path {
        load_from_path(path, &mut settings)?;
    } else {
        // Try to load from the executable's directory
        if let Ok(exe_dir) = std::env::current_exe() {
            let exe_dir = exe_dir.parent().unwrap_or_else(|| Path::new("."));
            let default_path = exe_dir.join("settings.toml");

            if let Ok(_) = load_from_path(default_path.to_str().unwrap(), &mut settings) {
                // Successfully loaded from the executable's directory
            } else {
                // Try to load from /usr/etc/g86-car-telemetry/speeduino-to-mqtt.toml
                if let Err(_) = load_from_path("/usr/etc/g86-car-telemetry/speeduino-to-mqtt.toml", &mut settings) {
                    // If loading from the first path fails, try the second path
                    load_from_path("/etc/g86-car-telemetry/speeduino-to-mqtt.toml", &mut settings)?;
                }
            }
        }
    }

    // Create an AppConfig struct by extracting values from the configuration.
    let mut app_config = AppConfig {
        port_name: settings
            .get_string("port_name")
            .expect("Missing port_name in configuration"),
        baud_rate: settings
            .get_int("baud_rate")
            .expect("Missing baud_rate in configuration"),
        mqtt_host: settings
            .get_string("mqtt_host")
            .expect("Missing mqtt_host in configuration"),
        mqtt_port: settings
            .get_int("mqtt_port")
            .expect("Missing mqtt_port in configuration"),
        mqtt_base_topic: settings
            .get_string("mqtt_base_topic")
            .expect("Missing mqtt_base_topic in configuration"),
        refresh_rate_ms: settings
            .get_int("refresh_rate_ms")
            .map(|value| value as u64)
            .ok(),
        config_path: config_path.map(|p| p.to_string()), // Convert &str to String
    };

    // If refresh_rate_ms is not specified in the config, use the default value (1000ms)
    if app_config.refresh_rate_ms.is_none() {
        app_config.refresh_rate_ms = Some(1000);
    }

    Ok(app_config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_load_configuration_from_file() {
        // Create a temporary settings.toml file for testing
        let toml_content = r#"
            port_name = "COM1"
            baud_rate = 9600
            mqtt_host = "mqtt.example.com"
            mqtt_port = 1883
            mqtt_base_topic = "sensors"
            refresh_rate_ms = 500
        "#;

        let temp_dir = tempdir().expect("Failed to create temporary directory");
        let file_path = temp_dir.path().join("settings.toml");

        fs::write(&file_path, toml_content).expect("Failed to write to temporary file");

        // Test the load_configuration function with the temporary file path
        let config = load_configuration(Some(file_path.to_str().unwrap())).expect("Failed to load configuration");

        // Check if the loaded configuration matches the expected values
        assert_eq!(config.port_name, "COM1");
        assert_eq!(config.baud_rate, 9600);
        assert_eq!(config.mqtt_host, "mqtt.example.com");
        assert_eq!(config.mqtt_port, 1883);
        assert_eq!(config.mqtt_base_topic, "sensors");
        assert_eq!(config.refresh_rate_ms, Some(500));
    }

    #[test]
    fn test_load_configuration_with_default_refresh_rate() {
        // Create a temporary settings.toml file for testing
        let toml_content = r#"
            port_name = "COM1"
            baud_rate = 9600
            mqtt_host = "mqtt.example.com"
            mqtt_port = 1883
            mqtt_base_topic = "sensors"
        "#;

        let temp_dir = tempdir().expect("Failed to create temporary directory");
        let file_path = temp_dir.path().join("settings.toml");

        fs::write(&file_path, toml_content).expect("Failed to write to temporary file");

        // Test the load_configuration function with the temporary file path
        let config = load_configuration(Some(file_path.to_str().unwrap())).expect("Failed to load configuration");

        // Check if the loaded configuration matches the expected values
        assert_eq!(config.port_name, "COM1");
        assert_eq!(config.baud_rate, 9600);
        assert_eq!(config.mqtt_host, "mqtt.example.com");
        assert_eq!(config.mqtt_port, 1883);
        assert_eq!(config.mqtt_base_topic, "sensors");
        assert_eq!(config.refresh_rate_ms, Some(1000)); // Default value
    }

    #[test]
    fn test_load_configuration_with_missing_file() {
        // Test the load_configuration function with a non-existent file path
        let result = load_configuration(Some("non_existent_file.toml"));

        // Check if the function returns an error
        assert!(result.is_err());
    }

    #[test]
    fn test_load_configuration_with_invalid_toml() {
        // Create a temporary invalid settings.toml file for testing
        let toml_content = r#"
            port_name = "COM1"
            baud_rate = "invalid_number"
        "#;

        let temp_dir = tempdir().expect("Failed to create temporary directory");
        let file_path = temp_dir.path().join("settings.toml");

        fs::write(&file_path, toml_content).expect("Failed to write to temporary file");

        // Test the load_configuration function with the temporary file path
        let result = load_configuration(Some(file_path.to_str().unwrap()));

        // Check if the function returns an error
        assert!(result.is_err());
    }
}