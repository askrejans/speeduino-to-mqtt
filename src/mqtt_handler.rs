use crate::config::AppConfig;
use paho_mqtt as mqtt;
use std::sync::Arc;

/// Set up and return an MQTT client based on the provided configuration.
///
/// This function takes an `AppConfig` reference, extracts MQTT-related information
/// (host and port) from it, creates an MQTT client, sets a timeout, and attempts to connect to the broker.
///
/// # Arguments
///
/// * `config` - A reference to the `AppConfig` struct containing MQTT configuration information.
///
/// # Returns
///
/// Returns a `Result` containing the MQTT client upon successful setup and connection or an error if the connection fails.
pub fn setup_mqtt(config: &Arc<AppConfig>) -> Result<mqtt::Client, String> {
    // Format the MQTT broker host and port.
    let host = format!("mqtt://{}:{}", config.mqtt_host, config.mqtt_port);

    // Create an MQTT client.
    let cli = match mqtt::Client::new(host) {
        Ok(client) => client,
        Err(err) => return Err(format!("Failed to create MQTT client: {}", err)),
    };

    // Use the `connect` method to connect to the broker.
    if let Err(err) = cli.connect(None) {
        return Err(format!("Failed to connect to MQTT broker: {}", err));
    }

    println!(
        "Connected to MQTT broker on {}:{}",
        config.mqtt_host, config.mqtt_port
    );

    Ok(cli) // Return the MQTT client after successful connection.
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_setup_mqtt() {
        // Create a dummy AppConfig for testing
        let config = Arc::new(AppConfig {
            port_name: String::from("COM1"),
            baud_rate: 9600,
            mqtt_host: String::from("test.example.com"),
            mqtt_port: 1883,
            mqtt_base_topic: String::from("sensors"),
            ..Default::default()
        });

        // Use a Mutex to ensure the test runs sequentially
        let mutex = Mutex::new(());
        let _guard = mutex.lock().unwrap();

        // Test the setup_mqtt function
        let result = setup_mqtt(&config);

        // Check if the result is an Err, indicating a connection failure
        match result {
            Ok(_) => panic!("Expected setup_mqtt to return Err, but it returned Ok."),
            Err(_) => {
                // Handle the error and exit with code 1
                eprintln!("Error: Failed to set up MQTT.");
                std::process::exit(1);
            }
        }
    }
}
