use crate::config::AppConfig;
use paho_mqtt as mqtt;
use std::process;
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
/// # Panics
///
/// Panics if there is an error creating the MQTT client or if it fails to connect to the broker.
///
/// # Returns
///
/// Returns an MQTT client upon successful setup and connection.
pub fn setup_mqtt(config: &Arc<AppConfig>) -> mqtt::Client {
    // Format the MQTT broker host and port.
    let host = format!("mqtt://{}:{}", config.mqtt_host, config.mqtt_port);

    // Create an MQTT client.
    let cli = mqtt::Client::new(host).unwrap_or_else(|e| {
        // Print an error message and exit the program if client creation fails.
        println!("Error creating the client: {:?}", e);
        process::exit(1);
    });

    // Use the `connect` method to connect to the broker.
    match cli.connect(None) {
        Ok(_) => {
            println!("Connected to MQTT broker");
            cli // Return the MQTT client after successful connection.
        }
        Err(e) => {
            println!("Failed to connect to MQTT broker: {:?}", e);
            process::exit(1);
        }
    }
}
