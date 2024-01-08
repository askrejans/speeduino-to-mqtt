mod config;
mod ecu_data_parser;
mod ecu_serial_comms_handler;
mod mqtt_handler;

use ecu_serial_comms_handler::start_ecu_communication;

#[tokio::main]
async fn main() {
    start_ecu_communication();
}
