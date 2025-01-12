use crate::config::AppConfig;
use paho_mqtt as mqtt;
use std::sync::Arc;

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

    // Parse the Realtime Data List
    let speeduino_data = parse_realtime_data(data);

    // Use the provided mqtt::Client instance for publishing
    publish_speeduino_params_to_mqtt(mqtt_client, config, &speeduino_data);
}

/// Combines two bytes into a single `u16` value.
///
/// This function takes two bytes, a low byte and a high byte, and combines
/// them into a single 16-bit unsigned integer. The low byte is placed in the
/// lower 8 bits of the result, and the high byte is placed in the upper 8 bits.
///
/// # Arguments
///
/// * `low` - The low byte.
/// * `high` - The high byte.
///
/// # Returns
///
/// A `u16` value that combines the low and high bytes.
///
/// # Example
///
/// ```
/// let low = 0x34;
/// let high = 0x12;
/// let combined = combine_bytes(low, high);
/// assert_eq!(combined, 0x1234);
/// ```
fn combine_bytes(high: u8, low: u8) -> u16 {
    ((high as u16) << 8) | (low as u16)
}

/// Parses the Realtime Data List and creates a `SpeeduinoData` instance.
///
/// This function reads a byte slice and extracts various fields to populate
/// a `SpeeduinoData` structure. It uses an internal helper function to read
/// individual bytes from the data slice.
///
/// # Arguments
///
/// * `data` - A byte slice containing the realtime data to be parsed.
///
/// # Returns
///
/// A `SpeeduinoData` instance populated with the parsed data.
///
/// # Example
///
/// ```
/// let data: &[u8] = &[0x01, 0x02, 0x03, ...];
/// let speeduino_data = parse_realtime_data(data);
/// ```
#[allow(unused_assignments)]
fn parse_realtime_data(data: &[u8]) -> SpeeduinoData {
    let mut offset = 0;

    fn read_byte(data: &[u8], offset: &mut usize) -> u8 {
        if *offset < data.len() {
            let value = data[*offset];
            *offset += 1;
            value
        } else {
            eprintln!("Not enough bytes remaining to read");
            0
        }
    }

    // Create a SpeeduinoData instance by reading each field
    SpeeduinoData {
        secl: read_byte(data, &mut offset),
        status1: read_byte(data, &mut offset),
        engine: read_byte(data, &mut offset),
        dwell: read_byte(data, &mut offset),
        map_low: read_byte(data, &mut offset),
        map_high: read_byte(data, &mut offset),
        mat: read_byte(data, &mut offset),
        coolant_adc: read_byte(data, &mut offset),
        bat_correction: read_byte(data, &mut offset),
        battery_10: read_byte(data, &mut offset),
        o2_primary: read_byte(data, &mut offset),
        ego_correction: read_byte(data, &mut offset),
        iat_correction: read_byte(data, &mut offset),
        wue_correction: read_byte(data, &mut offset),
        rpm_low: read_byte(data, &mut offset),
        rpm_high: read_byte(data, &mut offset),
        tae_amount: read_byte(data, &mut offset),
        corrections: read_byte(data, &mut offset),
        ve: read_byte(data, &mut offset),
        afr_target: read_byte(data, &mut offset),
        pw1_low: read_byte(data, &mut offset),
        pw1_high: read_byte(data, &mut offset),
        tps_dot: read_byte(data, &mut offset),
        advance: read_byte(data, &mut offset),
        tps: read_byte(data, &mut offset),
        loops_per_second_low: read_byte(data, &mut offset),
        loops_per_second_high: read_byte(data, &mut offset),
        free_ram_low: read_byte(data, &mut offset),
        free_ram_high: read_byte(data, &mut offset),
        boost_target: read_byte(data, &mut offset),
        boost_duty: read_byte(data, &mut offset),
        spark: read_byte(data, &mut offset),
        rpm_dot_low: read_byte(data, &mut offset),
        rpm_dot_high: read_byte(data, &mut offset),
        ethanol_pct: read_byte(data, &mut offset),
        flex_correction: read_byte(data, &mut offset),
        flex_ign_correction: read_byte(data, &mut offset),
        idle_load: read_byte(data, &mut offset),
        test_outputs: read_byte(data, &mut offset),
        o2_secondary: read_byte(data, &mut offset),
        baro: read_byte(data, &mut offset),
        canin: [
            read_byte(data, &mut offset),
            read_byte(data, &mut offset),
            read_byte(data, &mut offset),
            read_byte(data, &mut offset),
            read_byte(data, &mut offset),
            read_byte(data, &mut offset),
            read_byte(data, &mut offset),
            read_byte(data, &mut offset),
            read_byte(data, &mut offset),
            read_byte(data, &mut offset),
            read_byte(data, &mut offset),
            read_byte(data, &mut offset),
            read_byte(data, &mut offset),
            read_byte(data, &mut offset),
            read_byte(data, &mut offset),
            read_byte(data, &mut offset),
        ],
        tps_adc: read_byte(data, &mut offset),
        next_error: read_byte(data, &mut offset),
    }
}

/// Retrieves the parameters from the provided `SpeeduinoData` structure.
///
/// This function extracts various parameters from the `SpeeduinoData` structure
/// and returns them as a vector of tuples, where each tuple contains a parameter code
/// and its corresponding value as a string.
///
/// # Arguments
///
/// * `speeduino_data` - A reference to the `SpeeduinoData` structure containing the parameters.
///
/// # Returns
///
/// A vector of tuples, where each tuple contains a parameter code as a string slice
/// and its corresponding value as a string.
///
/// # Example
///
/// ```rust
/// let speeduino_data = SpeeduinoData { /* initialize fields */ };
/// let params = get_params_to_publish(&speeduino_data);
/// for (code, value) in params {
///     println!("{}: {}", code, value);
/// }
/// ```
fn get_params_to_publish(speeduino_data: &SpeeduinoData) -> Vec<(&str, String)> {
    vec![
        (
            "RPM",
            combine_bytes(speeduino_data.rpm_high, speeduino_data.rpm_low).to_string(),
        ),
        ("TPS", speeduino_data.tps.to_string()),
        ("VE1", speeduino_data.ve.to_string()),
        ("O2P", (speeduino_data.o2_primary as f32 / 10.0).to_string()),
        ("MAT", speeduino_data.mat.to_string()),
        ("CAD", speeduino_data.coolant_adc.to_string()),
        ("DWL", speeduino_data.dwell.to_string()),
        (
            "MAP",
            combine_bytes(speeduino_data.map_high, speeduino_data.map_low).to_string(),
        ),
        (
            "O2S",
            (speeduino_data.o2_secondary as f32 / 10.0).to_string(),
        ),
        ("ITC", speeduino_data.iat_correction.to_string()),
        ("TAE", speeduino_data.tae_amount.to_string()),
        ("COR", speeduino_data.corrections.to_string()),
        ("AFT", (speeduino_data.afr_target as f32 / 10.0).to_string()),
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
        ("BAT", (speeduino_data.battery_10 as f32 / 10.0).to_string()),
        ("EGC", speeduino_data.ego_correction.to_string()),
        ("WEC", speeduino_data.wue_correction.to_string()),
        ("SCL", speeduino_data.secl.to_string()),
    ]
}

/// Publishes Speeduino parameters to an MQTT broker.
///
/// This function retrieves the parameters from the provided `SpeeduinoData`
/// and publishes each parameter to the MQTT broker using the provided MQTT client.
///
/// # Arguments
///
/// * `client` - A reference to the MQTT client used to publish the messages.
/// * `config` - A reference to the application configuration, which contains the base MQTT topic.
/// * `speeduino_data` - A reference to the `SpeeduinoData` structure containing the parameters to be published.
///
/// # Example
///
/// ```rust
/// let client = mqtt::Client::new("mqtt://broker.hivemq.com:1883").unwrap();
/// let config = Arc::new(AppConfig { mqtt_base_topic: "speeduino/".to_string() });
/// let speeduino_data = SpeeduinoData { /* initialize fields */ };
///
/// publish_speeduino_params_to_mqtt(&client, &config, &speeduino_data);
/// ```
fn publish_speeduino_params_to_mqtt(
    client: &mqtt::Client,
    config: &Arc<AppConfig>,
    speeduino_data: &SpeeduinoData,
) {
    let params_to_publish = get_params_to_publish(speeduino_data);

    for (param_code, param_value) in params_to_publish {
        publish_param_to_mqtt(client, config, param_code, param_value);
    }
}

/// Helper function to publish a parameter to MQTT with a three-letter code.
///
/// This function constructs the MQTT topic using the base topic from the configuration
/// and the provided parameter code. It then publishes the parameter value to the MQTT broker.
///
/// # Arguments
///
/// * `client` - A reference to the MQTT client used to publish the message.
/// * `config` - A reference to the application configuration, which contains the base MQTT topic.
/// * `param_code` - A string slice representing the three-letter code of the parameter.
/// * `param_value` - A string containing the value of the parameter to be published.
///
/// # Example
///
/// ```rust
/// let client = mqtt::Client::new("mqtt://broker.hivemq.com:1883").unwrap();
/// let config = Arc::new(AppConfig { mqtt_base_topic: "speeduino/".to_string() });
/// let param_code = "RPM";
/// let param_value = "3000".to_string();
///
/// publish_param_to_mqtt(&client, &config, param_code, param_value);
/// ```
fn publish_param_to_mqtt(
    client: &mqtt::Client,
    config: &Arc<AppConfig>,
    param_code: &str,
    param_value: String,
) {
    let topic = format!("{}{}", config.mqtt_base_topic, param_code);
    let qos = 1;
    let message = mqtt::Message::new(&topic, param_value, qos);

    if let Err(e) = client.publish(message) {
        eprintln!("Failed to publish message to MQTT: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_realtime_data_valid() {
        let data: [u8; 41] = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
            0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C,
            0x1D, 0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29,
        ];

        let result = parse_realtime_data(&data);

        // Assert that all fields are correctly parsed
        assert_eq!(result.secl, 0x01);
        assert_eq!(result.status1, 0x02);
        assert_eq!(result.engine, 0x03);
        assert_eq!(result.dwell, 0x04);
        assert_eq!(result.map_low, 0x05);
        assert_eq!(result.map_high, 0x06);
        assert_eq!(result.mat, 0x07);
        assert_eq!(result.coolant_adc, 0x08);
        assert_eq!(result.bat_correction, 0x09);
        assert_eq!(result.battery_10, 0x0A);
        assert_eq!(result.o2_primary, 0x0B);
        assert_eq!(result.ego_correction, 0x0C);
        assert_eq!(result.iat_correction, 0x0D);
        assert_eq!(result.wue_correction, 0x0E);
        assert_eq!(result.rpm_low, 0x0F);
        assert_eq!(result.rpm_high, 0x10);
        assert_eq!(result.tae_amount, 0x11);
        assert_eq!(result.corrections, 0x12);
        assert_eq!(result.ve, 0x13);
        assert_eq!(result.afr_target, 0x14);
        assert_eq!(result.pw1_low, 0x15);
        assert_eq!(result.pw1_high, 0x16);
        assert_eq!(result.tps_dot, 0x17);
        assert_eq!(result.advance, 0x18);
        assert_eq!(result.tps, 0x19);
        assert_eq!(result.loops_per_second_low, 0x1A);
        assert_eq!(result.loops_per_second_high, 0x1B);
        assert_eq!(result.free_ram_low, 0x1C);
        assert_eq!(result.free_ram_high, 0x1D);
        assert_eq!(result.boost_target, 0x1E);
        assert_eq!(result.boost_duty, 0x1F);
        assert_eq!(result.spark, 0x20);
        assert_eq!(result.rpm_dot_low, 0x21);
        assert_eq!(result.rpm_dot_high, 0x22);
        assert_eq!(result.ethanol_pct, 0x23);
        assert_eq!(result.flex_correction, 0x24);
        assert_eq!(result.flex_ign_correction, 0x25);
        assert_eq!(result.idle_load, 0x26);
        assert_eq!(result.test_outputs, 0x27);
        assert_eq!(result.o2_secondary, 0x28);
        assert_eq!(result.baro, 0x29);
    }
}
