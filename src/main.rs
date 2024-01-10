/// # Speeduino To MQTT Processor
///
/// This Rust application serves as a processor for Speeduino data, converting it to MQTT messages.
/// It includes modules for configuration, ECU data parsing, serial communication handling, and MQTT handling.
/// The main function starts the ECU communication and displays a welcome message.
///
/// ## Usage
///
/// Simply run the application, and it will establish communication with the ECU. Press 'q' to quit the application.
///
/// ## Modules
///
/// - `config`: Module for configuration settings.
/// - `ecu_data_parser`: Module for parsing ECU data.
/// - `ecu_serial_comms_handler`: Module for handling serial communication with the ECU.
/// - `mqtt_handler`: Module for handling MQTT communication.
///
/// ## Functions
///
/// - `main()`: The main function that starts the ECU communication and displays the welcome message.
/// - `displayWelcome()`: Function to display a graphical welcome message.
mod config;
mod ecu_data_parser;
mod ecu_serial_comms_handler;
mod mqtt_handler;

use ecu_serial_comms_handler::start_ecu_communication;

/// Displays a graphical welcome message.
fn display_welcome() {
    println!("\nWelcome to Speeduino To MQTT Processor!\n");
    println!("===================================");
    println!("Press 'q' to quit the application.");
    println!("===================================\n");
}

#[tokio::main]
/// The main function that starts the ECU communication and displays the welcome message.
async fn main() {
    // Display welcome message
    display_welcome();

    // Start ECU communication
    start_ecu_communication();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_welcome() {
        display_welcome();
        // If the function runs without panicking, the test passes.
    }
}
