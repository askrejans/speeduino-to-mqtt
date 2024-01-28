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
use gumdrop::Options;
use ecu_serial_comms_handler::start_ecu_communication;
use crate::config::load_configuration;

/// Define options for the program.
#[derive(Debug, Options)]
struct MyOptions {
    #[options(help = "print help message")]
    help: bool,

    #[options(help = "Sets a custom config file", meta = "FILE")]
    config: Option<String>,
}

fn print_help() {
    println!("Usage: gps-to-mqtt [options]");
    println!("Options:");
    println!("  -h, --help               Print this help message");
    println!("  -c, --config FILE        Sets a custom config file path");
}

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

    // Parse CLI arguments using gumdrop
    let opts = MyOptions::parse_args_default_or_exit();

    if opts.help {
        // Use custom print_help function to display help and exit
        print_help();
        std::process::exit(0);
    }

    // Display welcome message
    display_welcome();

        // Load configuration, set up serial port, and start processing
        let config_path = opts.config.as_deref();
        let config = match load_configuration(config_path) {
            Ok(config) => config,
            Err(err) => {
                // Handle the error gracefully, print a message, and exit
                eprintln!("Error loading configuration: {}", err);
                std::process::exit(1);
            }
        };

    // Start ECU communication
    start_ecu_communication(config.clone());
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
