//! # Speeduino To MQTT Processor
//!
//! Production-ready async application that bridges Speeduino ECU data to MQTT.
//! Features structured logging, graceful shutdown, automatic reconnection,
//! and comprehensive error handling.

mod config;
mod ecu_data_parser;
mod ecu_serial_comms_handler;
mod errors;
mod mqtt_handler;

use crate::config::{load_configuration, AppConfig};
use crate::ecu_data_parser::process_speeduino_realtime_data;
use crate::ecu_serial_comms_handler::EcuSerialHandler;
use crate::mqtt_handler::{MqttHandler, MqttMessage};
use gumdrop::Options;
use futures_util::stream::StreamExt;
use signal_hook::consts::signal::*;
use signal_hook_tokio::Signals;
use std::sync::Arc;
use tokio::select;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;

/// CLI options
#[derive(Debug, Options)]
struct CliOptions {
    #[options(help = "print help message")]
    help: bool,

    #[options(help = "Sets a custom config file", meta = "FILE")]
    config: Option<String>,
}

/// Print usage help
fn print_help() {
    println!("Usage: speeduino-to-mqtt [options]");
    println!();
    println!("Options:");
    println!("  -h, --help               Print this help message");
    println!("  -c, --config FILE        Sets a custom config file path");
    println!();
    println!("Environment Variables:");
    println!("  SPEEDUINO_PORT_NAME      Override serial port");
    println!("  SPEEDUINO_MQTT_HOST      Override MQTT broker host");
    println!("  SPEEDUINO_LOG_LEVEL      Override log level (trace|debug|info|warn|error)");
    println!("  ... (all config options can be overridden with SPEEDUINO_ prefix)");
}

/// Display welcome banner
fn display_welcome() {
    println!("\x1b[1;32m"); // Green text
    println!("\nWelcome to Speeduino To MQTT Processor!");
    println!("========================================");

    // Car logo
    println!("\x1b[1;31m"); // Red text
    println!("       ______");
    println!("      //  ||\\ \\");
    println!(" ____//___||_\\ \\___");
    println!(" )  _          _    \\");
    println!(" |_/ \\________/ \\___|");
    println!("___\\_/________\\_/______");
    println!("\x1b[1;32m"); // Green text

    println!("========================================");
    println!("Version: {}", env!("CARGO_PKG_VERSION"));
    println!("========================================\n");
    println!("\x1b[0m"); // Reset color
}

/// Initialize logging based on configuration
fn init_logging(config: &AppConfig) {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.log_level));

    if config.log_json {
        // JSON formatted logs for production
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .json()
            .init();
    } else {
        // Pretty formatted logs for development
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_target(true)
            .init();
    }

    info!("Logging initialized at level: {}", config.log_level);
}

/// Handle system signals for graceful shutdown
async fn handle_signals(mut signals: Signals) {
    while let Some(signal) = signals.next().await {
        match signal {
            SIGTERM | SIGINT => {
                info!("Received shutdown signal ({}), initiating graceful shutdown", signal);
                break;
            }
            _ => {
                warn!("Received unhandled signal: {}", signal);
            }
        }
    }
}

/// Main ECU communication loop
async fn ecu_communication_loop(
    config: Arc<AppConfig>,
    mqtt_sender: mpsc::Sender<MqttMessage>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut serial_handler = EcuSerialHandler::new((*config).clone());
    
    // Initial connection
    info!("Connecting to ECU on {}", config.port_name);
    loop {
        match serial_handler.connect().await {
            Ok(_) => break,
            Err(e) => {
                error!("Failed to connect to ECU: {}", e);
                if serial_handler.get_retry_count() >= config.max_retry_count {
                    error!("Max connection attempts exceeded, exiting");
                    return Err(e.into());
                }
                serial_handler.reconnect().await?;
            }
        }
    }

    // Create polling interval
    let mut poll_interval = interval(Duration::from_millis(config.refresh_rate_ms));
    let mut consecutive_errors = 0;
    const MAX_CONSECUTIVE_ERRORS: u32 = 10;

    info!("Starting ECU data polling loop ({}ms interval)", config.refresh_rate_ms);

    loop {
        poll_interval.tick().await;

        // Check if device still exists
        if !serial_handler.check_device_exists() {
            warn!("ECU device disappeared, attempting reconnection");
            serial_handler.disconnect().await;
            
            match serial_handler.reconnect().await {
                Ok(_) => {
                    consecutive_errors = 0;
                    continue;
                }
                Err(e) => {
                    error!("Reconnection failed: {}", e);
                    consecutive_errors += 1;
                    
                    if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                        error!("Too many consecutive errors, exiting");
                        return Err(e.into());
                    }
                    continue;
                }
            }
        }

        // Read data from ECU
        match serial_handler.read_engine_data().await {
            Ok(data) => {
                debug!("Read {} bytes from ECU", data.len());
                
                // Parse and publish data
                if let Err(e) = process_speeduino_realtime_data(
                    &data,
                    &config,
                    &mqtt_sender,
                ).await {
                    error!("Failed to process ECU data: {}", e);
                    consecutive_errors += 1;
                } else {
                    // Reset error counter on success
                    if consecutive_errors > 0 {
                        consecutive_errors = 0;
                    }
                    serial_handler.reset_retry_count();
                }
            }
            Err(e) => {
                error!("Failed to read from ECU: {}", e);
                consecutive_errors += 1;
                
                if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                    error!("Too many consecutive read errors, attempting reconnection");
                    serial_handler.disconnect().await;
                    
                    if let Err(e) = serial_handler.reconnect().await {
                        error!("Reconnection failed: {}", e);
                        return Err(e.into());
                    }
                    consecutive_errors = 0;
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse CLI arguments
    let opts = CliOptions::parse_args_default_or_exit();

    if opts.help {
        print_help();
        std::process::exit(0);
    }

    // Display welcome banner
    display_welcome();

    // Load configuration
    info!("Loading configuration...");
    let config = match load_configuration(opts.config.as_deref()) {
        Ok(cfg) => Arc::new(cfg),
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };

    // Initialize logging
    init_logging(&config);
    
    // Display configuration summary
    config.display_summary();

    // Setup signal handlers for graceful shutdown
    let signals = Signals::new(&[SIGTERM, SIGINT])?;
    let signals_handle = signals.handle();
    let signals_task = tokio::spawn(handle_signals(signals));

    // Create MQTT handler
    info!("Setting up MQTT connection...");
    let mut mqtt_handler = match MqttHandler::new(config.clone()) {
        Ok(handler) => handler,
        Err(e) => {
            error!("Failed to create MQTT handler: {}", e);
            return Err(e.into());
        }
    };

    // Connect to MQTT broker
    if let Err(e) = mqtt_handler.connect().await {
        error!("Failed to connect to MQTT broker: {}", e);
        return Err(e.into());
    }

    // Get MQTT message sender
    let mqtt_sender = mqtt_handler.get_sender();

    // Start ECU communication loop
    let ecu_config = config.clone();
    let ecu_sender = mqtt_sender.clone();
    let ecu_task = tokio::spawn(async move {
        if let Err(e) = ecu_communication_loop(ecu_config, ecu_sender).await {
            error!("ECU communication loop failed: {}", e);
        }
    });

    info!("All tasks started successfully");
    info!("Press Ctrl+C to shutdown");

    // Wait for shutdown signal or task completion
    select! {
        _ = signals_task => {
            info!("Shutdown signal received");
        }
        _ = ecu_task => {
            warn!("ECU task terminated unexpectedly");
        }
        result = mqtt_handler.start_publishing_task() => {
            match result {
                Ok(_) => info!("MQTT task completed"),
                Err(e) => error!("MQTT task failed: {}", e),
            }
        }
    }

    // Cleanup
    info!("Shutting down gracefully...");
    signals_handle.close();
    
    info!("Goodbye!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_welcome() {
        // Should not panic
        display_welcome();
    }

    #[test]
    fn test_print_help() {
        // Should not panic
        print_help();
    }

    #[test]
    fn test_cli_options_parsing() {
        let args = vec!["program", "--help"];
        let opts = CliOptions::parse_args(&args[1..], gumdrop::ParsingStyle::default());
        assert!(opts.is_ok());
    }
}
