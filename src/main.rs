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
use crate::config::load_configuration;
use crate::config::AppConfig;
use ecu_serial_comms_handler::start_ecu_communication;
use gumdrop::Options;

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

/// Displays the welcome message and instructions for the Speeduino To MQTT Processor application.
///
/// This function performs the following tasks:
/// - Prints a welcome message in green.
/// - Displays a car logo in red.
/// - Provides instructions on how to use the application.
/// - Lists the available commands for the user.
fn display_welcome() {
    println!("\x1b[1;32m"); // Set text color to green
    println!("\nWelcome to Speeduino To MQTT Processor!\n");
    println!("===================================");

    // Display car logo in red
    println!("\x1b[1;31m"); // Set text color to red
    println!("       ______");
    println!("      //  ||\\ \\");
    println!(" ____//___||_\\ \\___");
    println!(" )  _          _    \\");
    println!(" |_/ \\________/ \\___|");
    println!("___\\_/________\\_/______");
    println!("\x1b[1;32m"); // Set text color back to green

    println!("===================================\n");

    println!(
        "This application processes data from Speeduino ECU and publishes it to an MQTT broker."
    );
    println!("Ensure your Speeduino ECU is connected and configured properly.");
    println!("Press 'q' to quit the application.");
    println!("===================================\n");

    // Display a list of available commands
    println!("Available Commands:");
    println!("q - Quit the application");
    println!("===================================\n");

    println!("\x1b[0m"); // Reset text color
}

#[tokio::main]
/// The main entry point of the application.
///
/// This function performs the following tasks:
/// - Parses command-line arguments.
/// - Displays a help message if requested.
/// - Displays a welcome message.
/// - Loads the configuration file.
/// - Starts ECU communication.
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

    // Load configuration
    let config = load_config_or_exit(opts.config.as_deref());

    // Start ECU communication
    start_ecu_communication(config);
}

/// Loads the configuration file or exits the application if an error occurs.
///
/// # Arguments
///
/// * `config_path` - An optional path to the configuration file.
///
/// # Returns
///
/// * `Config` - The loaded configuration.
fn load_config_or_exit(config_path: Option<&str>) -> AppConfig {
    match load_configuration(config_path) {
        Ok(config) => config,
        Err(err) => {
            // Handle the error gracefully, print a message, and exit
            eprintln!("Error loading configuration: {}", err);
            std::process::exit(1);
        }
    }
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
