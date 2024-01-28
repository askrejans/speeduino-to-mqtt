use crate::config::{load_configuration, AppConfig};
use crate::ecu_data_parser::process_speeduino_realtime_data;
use crate::mqtt_handler::setup_mqtt;
use lazy_static::lazy_static;
use serialport::SerialPort;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use atty::Stream;

lazy_static! {
    /// Interval between commands sent to the ECU.
    static ref COMMAND_INTERVAL: Duration = Duration::from_millis(
        load_configuration(None).unwrap().refresh_rate_ms.unwrap_or(1000)
    );

    /// Length of the engine data message.
    static ref ENGINE_DATA_MESSAGE_LENGTH: usize = 74; // Adjust the length based on the expected size
}

/// Set up and open a serial port based on the provided configuration.
///
/// # Arguments
///
/// * `config` - Reference to the `AppConfig` struct containing serial port configuration information.
///
/// # Returns
///
/// Returns a `Box` containing the opened serial port.
pub fn setup_serial_port(config: &AppConfig) -> Result<Box<dyn SerialPort>, serialport::Error> {
    println!(
        "Connecting to port: {}, baud rate: {}",
        config.port_name, config.baud_rate
    );
    serialport::new(&config.port_name, config.baud_rate as u32)
        .timeout(Duration::from_millis(1000))
        .open()
}

/// Read data from the provided serial port and process it.
///
/// This function continuously reads data from the serial port, processes the engine data,
/// and communicates with the MQTT broker based on the provided configuration.
pub fn start_ecu_communication(config: AppConfig) {

    let arc_config = Arc::new(config);

    // Setup MQTT outside the loop
    let mqtt_client = match setup_mqtt(&arc_config) {
        Ok(client) => client,
        Err(err) => {
            println!("Error setting up MQTT: {:?}", err);
            return;
        }
    };

    let port = match setup_serial_port(&arc_config) {
        Ok(port) => Arc::new(Mutex::new(port)),
        Err(err) => {
            println!("Error setting up serial port: {:?}", err);
            return;
        }
    };

    let (sender, receiver) = mpsc::channel(); // Create a channel for communication between threads
    let arc_sender = Arc::new(Mutex::new(sender));

    // Flag to indicate whether the program should exit
    let should_exit = Arc::new(Mutex::new(false));

    let arc_config_thread = arc_config.clone();

    thread::spawn({
        let mqtt_client = mqtt_client.clone();
        let port = port.clone();
        let should_exit = should_exit.clone();
    
        move || {
            let mut last_send_time = Instant::now();
            let mut connected = false; // Flag to track connection status
            println!("Connecting to Speeduino ECU..");
    
            loop {
                let elapsed_time = last_send_time.elapsed();
                if elapsed_time >= *COMMAND_INTERVAL {
                    // Read the entire engine data message length in the buffer
                    let engine_data = read_engine_data(&mut port.lock().unwrap());
    
                    // Process the engine data only if it's not empty
                    if !engine_data.is_empty() {
                        process_speeduino_realtime_data(&engine_data, &arc_config_thread, &mqtt_client);
    
                        // Print the connection message only if not connected
                        if !connected {
                            println!("Successfully connected to Speeduino ECU");
                            connected = true;
                        }
                    }
    
                    last_send_time = Instant::now();
                } else {
                    // Sleep for a short duration to avoid busy waiting
                    thread::sleep(Duration::from_millis(15));
                }
    
                // Check for a quit command from the main thread
                if let Ok(message) = receiver.try_recv() {
                    if message == "q" {
                        println!("Received quit command. Exiting the communication thread.");
                        break;
                    }
                }
    
                // Check if the main thread has signaled to exit
                if *should_exit.lock().unwrap() {
                    println!("Exiting the communication thread.");
                    break;
                }
            }
        }
    });
    

    let is_interactive = atty::is(Stream::Stdin);

    if is_interactive {
    // Add a loop in the main thread to handle user input
    loop {
        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .expect("Failed to read line");

        let trimmed_input = input.trim();

        // Send quit command to the communication thread
        arc_sender
            .lock()
            .unwrap()
            .send(trimmed_input.to_string())
            .unwrap();

        if trimmed_input.eq_ignore_ascii_case("q") {
            // Signal the communication thread to exit
            *should_exit.lock().unwrap() = true;

            println!("Shutting down. Goodbye!");
            // Terminate the entire program
            std::process::exit(0);
        } else {
            println!("Unknown command. Type 'q' to exit.");
        }
    }
}
}

/// Read the entire engine data message length in the buffer.
///
/// This function sends the "A" command to the ECU, reads data from the serial port,
/// and collects the engine data until the specified message length is reached.
///
/// # Arguments
///
/// * `port` - Mutable reference to the serial port.
///
/// # Returns
///
/// Returns a vector containing the engine data.
fn read_engine_data(port: &mut Box<dyn SerialPort>) -> Vec<u8> {
    let mut serial_buf: Vec<u8> = vec![0; 512]; // Adjust buffer size as needed
    let mut engine_data: Vec<u8> = Vec::new();

    // Send "A" command
    if let Err(e) = port.write_all("A".as_bytes()) {
        println!("Error sending command to the ECU: {:?}", e);
        return engine_data;
    }

    // Read available data from the serial port
    loop {
        match port.read(serial_buf.as_mut_slice()) {
            Ok(t) if t > 0 => {
                engine_data.extend_from_slice(&serial_buf[0..t]);

                // Check if the engine data message is complete
                if engine_data.len() >= *ENGINE_DATA_MESSAGE_LENGTH {
                    break;
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                println!("Read timed out");
                break;
            }
            Err(e) => {
                println!("{:?}", e);
                break;
            }
            Ok(_) => todo!(),
        }
    }

    engine_data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_setup_serial_port() {
        let config = AppConfig {
            port_name: String::from("/dev/ttyUSB0"),
            baud_rate: 9600,
            mqtt_host: String::from("test.example.com"),
            mqtt_port: 1883,
            mqtt_base_topic: String::from("speeduino"),
            ..Default::default()
        };

        let result = setup_serial_port(&config);
        assert!(result.is_ok());
    }
}
