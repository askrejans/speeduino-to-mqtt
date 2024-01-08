use crate::config::load_configuration;
use crate::config::AppConfig;
use crate::ecu_data_parser::process_speeduino_realtime_data;
use crate::mqtt_handler::setup_mqtt;
use paho_mqtt as mqtt;
use serialport::SerialPort;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const COMMAND_INTERVAL: Duration = Duration::from_millis(100); // 10Hz

/// Set up and open a serial port based on the provided configuration.
pub fn setup_serial_port(config: &AppConfig) -> Box<dyn SerialPort> {
    serialport::new(&config.port_name, config.baud_rate as u32)
        .timeout(Duration::from_millis(1000))
        .open()
        .expect("Failed to open port")
}

/// Read data from the provided serial port and process it.
pub fn start_ecu_communication() {
    let config = load_configuration();
    let arc_config = Arc::new(config); // Wrap AppConfig in Arc

    // Setup MQTT outside the loop
    let mqtt_client = setup_mqtt(&arc_config);

    let port = setup_serial_port(&arc_config);
    let port = Arc::new(Mutex::new(port));

    let mut last_send_time = Instant::now();

    loop {
        let elapsed_time = last_send_time.elapsed();
        if elapsed_time >= COMMAND_INTERVAL {
            // Pass the MQTT client instance to the send_and_read_from_port function
            send_and_read_from_port(&mut port.lock().unwrap(), &arc_config, &mqtt_client);
            last_send_time = Instant::now();
        }
    }
}

/// Read data from the provided serial port, process it, and publish to MQTT.
fn send_and_read_from_port(
    port: &mut Box<dyn SerialPort>,
    config: &Arc<AppConfig>,
    mqtt_client: &mqtt::Client,
) {
    let mut serial_buf: Vec<u8> = vec![0; 256];

    let elapsed_time = Instant::now();

    // Send "A" command
    if let Err(e) = port.write_all("A".as_bytes()) {
        eprintln!("Error sending command to the ECU: {:?}", e);
    }
    // Read and process data
    match port.read(serial_buf.as_mut_slice()) {
        Ok(t) if t > 0 => {
            let data = &serial_buf[0..t];
            process_speeduino_realtime_data(data, config, mqtt_client);
        }
        Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => println!("Read timed out"),
        Err(e) => eprintln!("{:?}", e),
        Ok(_) => todo!(),
    }

    // Sleep to maintain the 10Hz interval
    let sleep_time = match COMMAND_INTERVAL.checked_sub(elapsed_time.elapsed()) {
        Some(time) => time,
        None => Duration::from_secs(0),
    };
    if sleep_time > Duration::from_secs(0) {
        thread::sleep(sleep_time);
    }
}
