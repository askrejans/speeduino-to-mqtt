use crate::config::{load_configuration, AppConfig};
use crate::ecu_data_parser::process_speeduino_realtime_data;
use crate::mqtt_handler::setup_mqtt;
use atty::Stream;
use lazy_static::lazy_static;
use paho_mqtt as mqtt;
use serialport::SerialPort;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

lazy_static! {
    /// Interval between commands sent to the ECU.
    static ref COMMAND_INTERVAL: Duration = Duration::from_millis(
        load_configuration(None).unwrap().refresh_rate_ms.unwrap_or(1000)
    );

    /// Length of the engine data message.
    /// Response length setfor current Speeduino firmware
    static ref ENGINE_DATA_MESSAGE_LENGTH: usize = 120;
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

    let port = serialport::new(&config.port_name, config.baud_rate as u32)
        .timeout(Duration::from_millis(1000))
        .open();

    match port {
        Ok(p) => Ok(p),
        Err(e) => {
            eprintln!("Failed to open serial port: {}", e);
            Err(e)
        }
    }
}

/// Starts the ECU communication process.
///
/// This function initializes the necessary components for communication with the Speeduino ECU,
/// including setting up the MQTT client and serial port. It then spawns a separate thread to handle
/// the communication with the ECU and processes user input to control the communication thread.
///
/// # Arguments
///
/// * `config` - The application configuration containing settings for the serial port and MQTT client.
///
/// # Behavior
///
/// The function performs the following steps:
/// 1. Creates an `Arc` for the application configuration.
/// 2. Sets up the MQTT client and handles any errors that occur during setup.
/// 3. Sets up the serial port and handles any errors that occur during setup.
/// 4. Creates a channel for communication between the main thread and the communication thread.
/// 5. Spawns a separate thread to handle the communication with the ECU.
/// 6. Handles user input from the command line to control the communication thread.
///
/// If the program is running interactively (i.e., attached to a terminal), it will continuously
/// read lines from the standard input. If the input is "q", it will send a quit command to the
/// communication thread and set the `should_exit` flag to true, then terminate the program.
/// If the input is not recognized, it will prompt the user to type "q" to exit.
///
/// If the program is not running interactively (i.e., running as a service), it will run an
/// empty loop to keep the program active.
pub fn start_ecu_communication(config: AppConfig) {
    let arc_config = Arc::new(config);

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

    let (sender, receiver) = mpsc::channel();
    let arc_sender = Arc::new(Mutex::new(sender));
    let should_exit = Arc::new(Mutex::new(false));

    let arc_config_thread = arc_config.clone();
    let mqtt_client_thread = mqtt_client.clone();
    let port_thread = port.clone();
    let should_exit_thread = should_exit.clone();

    thread::spawn(move || {
        communication_thread(
            mqtt_client_thread,
            port_thread,
            arc_config_thread,
            receiver,
            should_exit_thread,
        );
    });

    handle_user_input(arc_sender, should_exit);
}

fn check_device_exists(port_name: &str) -> bool {
    serialport::available_ports()
        .map(|ports| ports.iter().any(|p| p.port_name == port_name))
        .unwrap_or(false)
}

fn wait_for_device(port_name: &str) -> bool {
    let max_attempts = 5;
    let mut attempts = 0;

    while attempts < max_attempts {
        if check_device_exists(port_name) {
            return true;
        }
        println!(
            "Waiting for device {}... (attempt {}/{})",
            port_name,
            attempts + 1,
            max_attempts
        );
        thread::sleep(Duration::from_secs(1));
        attempts += 1;
    }
    false
}

/// Handles the communication with the Speeduino ECU.
///
/// This function runs in a separate thread and continuously communicates with the Speeduino ECU.
/// It reads engine data at regular intervals, processes the data, and sends it to the MQTT client.
/// It also listens for quit commands from the main thread and exits the loop when a quit command is received.
///
/// # Arguments
///
/// * `mqtt_client` - The MQTT client used to publish engine data.
/// * `port` - A thread-safe reference to the serial port used for communication with the ECU.
/// * `arc_config` - A thread-safe reference to the application configuration.
/// * `receiver` - A channel receiver used to receive messages from the main thread.
/// * `should_exit` - A thread-safe flag that indicates whether the communication thread should exit.
///
/// # Behavior
///
/// The function enters a loop where it performs the following actions:
/// 1. Checks if the elapsed time since the last send is greater than or equal to the command interval.
/// 2. Reads engine data from the serial port.
/// 3. Processes the engine data and sends it to the MQTT client if the data is not empty.
/// 4. Prints a connection message if the connection to the ECU is successful.
/// 5. Sleeps for a short duration to avoid busy waiting.
/// 6. Checks for a quit command from the main thread and exits the loop if a quit command is received.
/// 7. Checks if the main thread has signaled to exit and exits the loop if the flag is set.
fn communication_thread(
    mqtt_client: mqtt::Client,
    port: Arc<Mutex<Box<dyn SerialPort>>>,
    arc_config: Arc<AppConfig>,
    receiver: mpsc::Receiver<String>,
    should_exit: Arc<Mutex<bool>>,
) {
    let mut last_send_time = Instant::now();
    let mut connected = false;
    println!("Connecting to Speeduino ECU..");

    loop {
        if *should_exit.lock().unwrap() {
            println!("Exiting the communication thread.");
            break;
        }

        // Check if device exists
        if !check_device_exists(&arc_config.port_name) {
            if connected {
                println!(
                    "Lost connection to {} - waiting for device to return...",
                    arc_config.port_name
                );
                connected = false;
            }

            // Wait for device to return
            if !wait_for_device(&arc_config.port_name) {
                thread::sleep(Duration::from_secs(5));
                continue;
            }

            // Try to reopen port
            match setup_serial_port(&arc_config) {
                Ok(new_port) => {
                    let mut port_guard = port.lock().unwrap();
                    *port_guard = new_port;
                    println!("Reconnected to {}", arc_config.port_name);
                }
                Err(e) => {
                    eprintln!("Failed to reopen port: {}", e);
                    thread::sleep(Duration::from_secs(5));
                    continue;
                }
            }
        }

        let elapsed_time = last_send_time.elapsed();
        if elapsed_time >= *COMMAND_INTERVAL {
            let mut port_guard = port.lock().unwrap();
            let engine_data = read_engine_data(&mut port_guard);

            if !engine_data.is_empty() {
                process_speeduino_realtime_data(&engine_data, &arc_config, &mqtt_client);

                if !connected {
                    println!("Successfully connected to Speeduino ECU");
                    connected = true;
                }
            } else {
                // Empty data might indicate connection issues
                if connected {
                    println!("Lost connection - will try to reconnect...");
                    connected = false;
                }
            }

            last_send_time = Instant::now();
        } else {
            thread::sleep(Duration::from_millis(15));
        }

        if let Ok(message) = receiver.try_recv() {
            if message == "q" {
                println!("Received quit command. Exiting the communication thread.");
                break;
            }
        }
    }
}

/// Handles user input from the command line.
///
/// This function runs in the main thread and listens for user input from the command line.
/// If the input is "q", it signals the communication thread to exit and terminates the program.
/// If the input is not recognized, it prompts the user to type "q" to exit.
///
/// # Arguments
///
/// * `arc_sender` - An `Arc<Mutex<mpsc::Sender<String>>>` used to send messages to the communication thread.
/// * `should_exit` - An `Arc<Mutex<bool>>` flag that indicates whether the program should exit.
///
/// # Behavior
///
/// If the program is running interactively (i.e., attached to a terminal), it will continuously
/// read lines from the standard input. If the input is "q", it will send a quit command to the
/// communication thread and set the `should_exit` flag to true, then terminate the program.
/// If the input is not recognized, it will prompt the user to type "q" to exit.
///
/// If the program is not running interactively (i.e., running as a service), it will run an
/// empty loop to keep the program active.
fn handle_user_input(arc_sender: Arc<Mutex<mpsc::Sender<String>>>, should_exit: Arc<Mutex<bool>>) {
    let is_interactive = atty::is(Stream::Stdin);

    if is_interactive {
        loop {
            let mut input = String::new();
            match std::io::stdin().read_line(&mut input) {
                Ok(0) => break,
                Ok(_) => {
                    let trimmed_input = input.trim();
                    arc_sender
                        .lock()
                        .unwrap()
                        .send(trimmed_input.to_string())
                        .unwrap();

                    if trimmed_input.eq_ignore_ascii_case("q") {
                        *should_exit.lock().unwrap() = true;
                        println!("Shutting down. Goodbye!");
                        std::process::exit(0);
                    } else {
                        println!("Unknown command. Type 'q' to exit.");
                    }
                }
                Err(err) => {
                    eprintln!("Error reading input: {}", err);
                    break;
                }
            }
        }
    } else {
        loop {
            thread::sleep(Duration::from_millis(15));
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
    let mut engine_data: Vec<u8> = Vec::with_capacity(120);

    // Clear buffers
    if let Err(e) = port.clear(serialport::ClearBuffer::All) {
        eprintln!("Failed to clear buffers: {:?}", e);
    }

    // Set timeout
    if let Err(e) = port.set_timeout(Duration::from_millis(2000)) {
        eprintln!("Failed to set timeout: {:?}", e);
        return engine_data;
    }

    // Send single 'A' byte
    if let Err(e) = port.write_all(&[b'A']) {
        eprintln!("Error sending command: {:?}", e);
        return engine_data;
    }

    // Wait for processing
    thread::sleep(Duration::from_millis(100));

    // Read exact number of bytes
    let mut buffer = vec![0u8; 120];
    match port.read_exact(&mut buffer) {
        Ok(_) => {
            engine_data.extend_from_slice(&buffer);
        }
        Err(e) => {
            eprintln!("Failed to read data: {:?}", e);
        }
    }

    engine_data
}
