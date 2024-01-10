use config::Config;

/// Struct to hold the application configuration.
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
        }
    }
}

/// Load application configuration from a TOML file.
///
/// This function reads the configuration settings from a TOML file named "settings.toml".
/// It expects the following keys in the TOML file: "port_name", "baud_rate", "mqtt_host", "mqtt_port", and "mqtt_base_topic".
///
/// # Panics
/// Panics if any of the required configuration keys are missing or if there is an error reading the configuration file.
///
/// # Returns
/// Returns an `AppConfig` struct containing the loaded configuration.
pub fn load_configuration() -> AppConfig {
    // Build a new Config object with a file source.
    let config_file_path =
        std::env::var("CONFIG_FILE_PATH").unwrap_or_else(|_| String::from("settings.toml"));

    let settings = Config::builder()
        .add_source(config::File::with_name(&config_file_path))
        .build()
        .expect("Failed to build configuration");

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
    };
    // If refresh_rate_ms is not specified in the config, use the default value (1000ms)
    if app_config.refresh_rate_ms.is_none() {
        app_config.refresh_rate_ms = Some(1000);
    }

    app_config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_configuration() {
        // Create a temporary settings.toml file for testing
        let toml_content = r#"
            port_name = "COM1"
            baud_rate = 9600
            mqtt_host = "mqtt.example.com"
            mqtt_port = 1883
            mqtt_base_topic = "sensors"
        "#;

        let temp_dir =
            tempdir::TempDir::new("config_test").expect("Failed to create temporary directory");
        let file_path = temp_dir.path().join("settings.toml");

        std::fs::write(&file_path, toml_content).expect("Failed to write to temporary file");

        // Set CONFIG_FILE_PATH environment variable to point to the temporary file
        std::env::set_var("CONFIG_FILE_PATH", file_path.to_str().unwrap());

        // Test the load_configuration function
        let config = load_configuration();

        // Check if the loaded configuration matches the expected values
        assert_eq!(config.port_name, "COM1");
        assert_eq!(config.baud_rate, 9600);
        assert_eq!(config.mqtt_host, "mqtt.example.com");
        assert_eq!(config.mqtt_port, 1883);
        assert_eq!(config.mqtt_base_topic, "sensors");
    }
}
