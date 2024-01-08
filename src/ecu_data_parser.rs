use crate::config::AppConfig;
use crate::mqtt_handler::setup_mqtt;
use paho_mqtt as mqtt;
use std::sync::Arc;
use tokio::task;

/// Represents the Speeduino ECU data structure.
#[derive(Debug)]
struct SpeeduinoData {
    secl: u8,                  // Counter for +1s
    status1: u8,               // Status byte 1
    engine: u8,                // Engine status
    dwell: u8,                 // Dwell time
    map_low: u8,               // Low byte of MAP sensor reading
    map_high: u8,              // High byte of MAP sensor reading
    mat: u8,                   // Manifold Air Temperature sensor reading
    coolant_adc: u8,           // Coolant Analog-to-Digital Conversion value
    bat_correction: u8,        // Battery correction
    battery_10: u8,            // Battery voltage * 10
    o2_primary: u8,            // Primary O2 sensor reading
    ego_correction: u8,        // EGO Correction
    iat_correction: u8,        // IAT Correction
    wue_correction: u8,        // Warm-Up Enrichment Correction
    rpm_low: u8,               // Low byte of RPM
    rpm_high: u8,              // High byte of RPM
    tae_amount: u8,            // TAE Amount
    corrections: u8,           // Corrections
    ve: u8,                    // Volumetric Efficiency
    afr_target: u8,            // AFR Target
    pw1_low: u8,               // Low byte of Pulse Width 1
    pw1_high: u8,              // High byte of Pulse Width 1
    tps_dot: u8,               // Throttle Position Sensor change per second
    advance: u8,               // Ignition Advance
    tps: u8,                   // Throttle Position Sensor reading
    loops_per_second_low: u8,  // Low byte of loops per second
    loops_per_second_high: u8, // High byte of loops per second
    free_ram_low: u8,          // Low byte of free RAM
    free_ram_high: u8,         // High byte of free RAM
    boost_target: u8,          // Boost Target
    boost_duty: u8,            // Boost Duty
    spark: u8,                 // Spark
    rpm_dot_low: u8,           // Low byte of RPM DOT (assuming signed integer)
    rpm_dot_high: u8,          // High byte of RPM DOT (assuming signed integer)
    ethanol_pct: u8,           // Ethanol Percentage
    flex_correction: u8,       // Flex Fuel Correction
    flex_ign_correction: u8,   // Flex Fuel Ignition Correction
    idle_load: u8,             // Idle Load
    test_outputs: u8,          // Test Outputs
    o2_secondary: u8,          // Secondary O2 sensor reading
    baro: u8,                  // Barometric Pressure
    canin: [u8; 16],           // CAN Input values
    tps_adc: u8,               // Throttle Position Sensor ADC value
    next_error: u8,            // Next Error
}

/// Process and print the received Speeduino ECU data
///
/// # Arguments
///
/// * `data` - A slice of bytes representing received data.
/// * `config` - The Arc<AppConfig> instance.
/// * `mqtt_client` - The mqtt::Client instance.
pub fn process_speeduino_realtime_data(
    data: &[u8],
    config: &Arc<AppConfig>,
    mqtt_client: &mqtt::Client,
) {
    // Ensure that the received data is at least of the expected minimum size
    if data.len() < 3 {
        eprintln!("Invalid data received. Expected at least 3 bytes.");
        return;
    }

    // Confirming the received instruction
    let confirmation_byte = data[0];
    if confirmation_byte != 0x41 {
        eprintln!("Invalid confirmation byte received. Expected A");
        return;
    }

    // Extracting the Realtime Data List
    let realtime_data = &data[1..];
    // Parse the Realtime Data List
    let speeduino_data = parse_realtime_data(realtime_data);

    // Use the provided mqtt::Client instance for publishing
    publish_speeduino_params_to_mqtt(mqtt_client, config, &speeduino_data);
}

fn combine_bytes(high: u8, low: u8) -> u16 {
    ((high as u16) << 8) | (low as u16)
}

/// Parse the Realtime Data List and create a SpeeduinoData instance
#[allow(unused_assignments)]
fn parse_realtime_data(data: &[u8]) -> SpeeduinoData {
    let mut offset = 0;

    macro_rules! read_byte {
        () => {{
            if offset < data.len() {
                let value = data[offset];
                offset += 1;
                value
            } else {
                eprintln!("Not enough bytes remaining to read");
                0
            }
        }};
    }

    // Other macro_rules for reading bytes, signed bytes, and canin array go here...

    // Create a SpeeduinoData instance by reading each field
    SpeeduinoData {
        secl: read_byte!(),
        status1: read_byte!(),
        engine: read_byte!(),
        dwell: read_byte!(),
        map_low: read_byte!(),
        map_high: read_byte!(),
        mat: read_byte!(),
        coolant_adc: read_byte!(),
        bat_correction: read_byte!(),
        battery_10: read_byte!(),
        o2_primary: read_byte!(),
        ego_correction: read_byte!(),
        iat_correction: read_byte!(),
        wue_correction: read_byte!(),
        rpm_low: read_byte!(),
        rpm_high: read_byte!(),
        tae_amount: read_byte!(),
        corrections: read_byte!(),
        ve: read_byte!(),
        afr_target: read_byte!(),
        pw1_low: read_byte!(),
        pw1_high: read_byte!(),
        tps_dot: read_byte!(),
        advance: read_byte!(),
        tps: read_byte!(),
        loops_per_second_low: read_byte!(),
        loops_per_second_high: read_byte!(),
        free_ram_low: read_byte!(),
        free_ram_high: read_byte!(),
        boost_target: read_byte!(),
        boost_duty: read_byte!(),
        spark: read_byte!(),
        rpm_dot_low: read_byte!(),
        rpm_dot_high: read_byte!(),
        ethanol_pct: read_byte!(),
        flex_correction: read_byte!(),
        flex_ign_correction: read_byte!(),
        idle_load: read_byte!(),
        test_outputs: read_byte!(),
        o2_secondary: read_byte!(),
        baro: read_byte!(),
        canin: [
            read_byte!(),
            read_byte!(),
            read_byte!(),
            read_byte!(),
            read_byte!(),
            read_byte!(),
            read_byte!(),
            read_byte!(),
            read_byte!(),
            read_byte!(),
            read_byte!(),
            read_byte!(),
            read_byte!(),
            read_byte!(),
            read_byte!(),
            read_byte!(),
        ],
        tps_adc: read_byte!(),
        next_error: read_byte!(),
    }
}

/// Helper function to publish Speeduino parameters to MQTT
fn publish_speeduino_params_to_mqtt(
    client: &mqtt::Client,
    config: &Arc<AppConfig>,
    speeduino_data: &SpeeduinoData,
) {
    // List of parameters to publish
    let params_to_publish: Vec<(&str, String)> = vec![
        (
            "RPM",
            combine_bytes(speeduino_data.rpm_high, speeduino_data.rpm_low).to_string(),
        ),
        ("TPS", speeduino_data.tps.to_string()),
        ("VE", speeduino_data.ve.to_string()),
        ("O2P", speeduino_data.o2_primary.to_string()),
        ("MAT", speeduino_data.mat.to_string()),
        ("CAD", speeduino_data.coolant_adc.to_string()),
        ("DWL", speeduino_data.dwell.to_string()),
        (
            "MAP",
            combine_bytes(speeduino_data.map_high, speeduino_data.map_low).to_string(),
        ),
        ("O2S", speeduino_data.o2_secondary.to_string()),
        ("ITC", speeduino_data.iat_correction.to_string()),
        ("TAE", speeduino_data.tae_amount.to_string()),
        ("COR", speeduino_data.corrections.to_string()),
        ("AFT", speeduino_data.afr_target.to_string()),
        (
            "PW1",
            combine_bytes(speeduino_data.pw1_high, speeduino_data.pw1_low).to_string(),
        ),
        ("TPD", speeduino_data.tps_dot.to_string()),
        ("ADV", speeduino_data.advance.to_string()),
        (
            "LPS",
            combine_bytes(
                speeduino_data.loops_per_second_high,
                speeduino_data.loops_per_second_low,
            )
            .to_string(),
        ),
        (
            "FRM",
            combine_bytes(speeduino_data.free_ram_high, speeduino_data.free_ram_low).to_string(),
        ),
        ("BST", speeduino_data.boost_target.to_string()),
        ("BSD", speeduino_data.boost_duty.to_string()),
        ("SPK", speeduino_data.spark.to_string()),
        (
            "RPD",
            combine_bytes(speeduino_data.rpm_dot_high, speeduino_data.rpm_dot_low).to_string(),
        ),
        ("ETH", speeduino_data.ethanol_pct.to_string()),
        ("FLC", speeduino_data.flex_correction.to_string()),
        ("FIC", speeduino_data.flex_ign_correction.to_string()),
        ("ILL", speeduino_data.idle_load.to_string()),
        ("TOF", speeduino_data.test_outputs.to_string()),
        ("BAR", speeduino_data.baro.to_string()),
        (
            "CN1",
            combine_bytes(speeduino_data.canin[1], speeduino_data.canin[0]).to_string(),
        ),
        (
            "CN2",
            combine_bytes(speeduino_data.canin[3], speeduino_data.canin[2]).to_string(),
        ),
        (
            "CN3",
            combine_bytes(speeduino_data.canin[5], speeduino_data.canin[4]).to_string(),
        ),
        (
            "CN4",
            combine_bytes(speeduino_data.canin[7], speeduino_data.canin[6]).to_string(),
        ),
        (
            "CN5",
            combine_bytes(speeduino_data.canin[9], speeduino_data.canin[8]).to_string(),
        ),
        (
            "CN6",
            combine_bytes(speeduino_data.canin[11], speeduino_data.canin[10]).to_string(),
        ),
        (
            "CN7",
            combine_bytes(speeduino_data.canin[13], speeduino_data.canin[12]).to_string(),
        ),
        (
            "CN8",
            combine_bytes(speeduino_data.canin[15], speeduino_data.canin[14]).to_string(),
        ),
        ("TAD", speeduino_data.tps_adc.to_string()),
        ("NER", speeduino_data.next_error.to_string()),
        ("STA", speeduino_data.status1.to_string()),
        ("ENG", speeduino_data.engine.to_string()),
        ("BTC", speeduino_data.bat_correction.to_string()),
        ("BAT", speeduino_data.battery_10.to_string()),
        ("EGC", speeduino_data.ego_correction.to_string()),
        ("WEC", speeduino_data.wue_correction.to_string()),
        ("SCL", speeduino_data.secl.to_string()),
    ];

    // Iterate over parameters and publish to MQTT
    for (param_code, param_value) in params_to_publish {
        publish_param_to_mqtt(client, config, param_code, param_value);
    }
}

/// Helper function to publish a parameter to MQTT with a three-letter code
fn publish_param_to_mqtt(
    client: &mqtt::Client,
    config: &Arc<AppConfig>,
    param_code: &str,
    param_value: String,
) {
    // Concatenate the three-letter code to the base MQTT topic
    let topic = format!("{}{}", config.mqtt_base_topic, param_code);

    // Specify the desired QoS level
    let qos = 1; // Specify the desired QoS level, adjust as needed

    // Create a message and publish it to the MQTT topic
    let message = mqtt::Message::new(&topic, param_value, qos);
    client
        .publish(message)
        .expect("Failed to publish message to MQTT");
}
