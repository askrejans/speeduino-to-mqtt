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

    // Extracting the Realtime Data List
    let realtime_data = &data[0..];
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
        // RPM: Engine revolutions per minute
        (
            "RPM",
            combine_bytes(speeduino_data.rpm_high, speeduino_data.rpm_low).to_string(),
        ),
        // TPS: Throttle Position Sensor reading (0% to 100%)
        ("TPS", speeduino_data.tps.to_string()),
        // VE: Volumetric Efficiency (%)
        ("VE1", speeduino_data.ve.to_string()),
        // O2P: Primary O2 sensor reading
        ("O2P", (speeduino_data.o2_primary as f32 / 10.0).to_string()),
        // MAT: Manifold Air Temperature sensor reading
        ("MAT", speeduino_data.mat.to_string()),
        // CAD: Coolant Analog-to-Digital Conversion value
        ("CAD", speeduino_data.coolant_adc.to_string()),
        // DWL: Dwell time
        ("DWL", speeduino_data.dwell.to_string()),
        // MAP: Manifold Absolute Pressure sensor reading
        (
            "MAP",
            combine_bytes(speeduino_data.map_high, speeduino_data.map_low).to_string(),
        ),
        // O2S: Secondary O2 sensor reading
        (
            "O2S",
            (speeduino_data.o2_secondary as f32 / 10.0).to_string(),
        ),
        // ITC: Manifold Air Temperature Correction (%)
        ("ITC", speeduino_data.iat_correction.to_string()),
        // TAE: Warm-Up Enrichment Correction (%)
        ("TAE", speeduino_data.tae_amount.to_string()),
        // COR: Total GammaE (%)
        ("COR", speeduino_data.corrections.to_string()),
        // AFT: Air-Fuel Ratio Target
        ("AFT", (speeduino_data.afr_target as f32 / 10.0).to_string()),
        // PW1: Pulse Width 1
        (
            "PW1",
            combine_bytes(speeduino_data.pw1_high, speeduino_data.pw1_low).to_string(),
        ),
        // TPD: Throttle Position Sensor Change per Second
        ("TPD", speeduino_data.tps_dot.to_string()),
        // ADV: Ignition Advance
        ("ADV", speeduino_data.advance.to_string()),
        // LPS: Loops per Second
        (
            "LPS",
            combine_bytes(
                speeduino_data.loops_per_second_high,
                speeduino_data.loops_per_second_low,
            )
            .to_string(),
        ),
        // FRM: Free RAM
        (
            "FRM",
            combine_bytes(speeduino_data.free_ram_high, speeduino_data.free_ram_low).to_string(),
        ),
        // BST: Boost Target
        ("BST", speeduino_data.boost_target.to_string()),
        // BSD: Boost Duty
        ("BSD", speeduino_data.boost_duty.to_string()),
        // SPK: Spark
        ("SPK", speeduino_data.spark.to_string()),
        // RPD: RPM DOT (assuming signed integer)
        (
            "RPD",
            combine_bytes(speeduino_data.rpm_dot_high, speeduino_data.rpm_dot_low).to_string(),
        ),
        // ETH: Ethanol Percentage
        ("ETH", speeduino_data.ethanol_pct.to_string()),
        // FLC: Flex Fuel Correction
        ("FLC", speeduino_data.flex_correction.to_string()),
        // FIC: Flex Fuel Ignition Correction
        ("FIC", speeduino_data.flex_ign_correction.to_string()),
        // ILL: Idle Load
        ("ILL", speeduino_data.idle_load.to_string()),
        // TOF: Test Outputs
        ("TOF", speeduino_data.test_outputs.to_string()),
        // BAR: Barometric Pressure
        ("BAR", speeduino_data.baro.to_string()),
        // CN1 to CN8: CAN Input values (Combine bytes)
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
        // TAD: Throttle Position Sensor ADC value
        ("TAD", speeduino_data.tps_adc.to_string()),
        // NER: Next Error code
        ("NER", speeduino_data.next_error.to_string()),
        // STA: Status 1
        ("STA", speeduino_data.status1.to_string()),
        // ENG: Engine status
        ("ENG", speeduino_data.engine.to_string()),
        // BTC: Battery Temperature Correction
        ("BTC", speeduino_data.bat_correction.to_string()),
        // BAT: Battery voltage (scaled by 10)
        ("BAT", (speeduino_data.battery_10 as f32 / 10.0).to_string()),
        // EGC: EGO Correction
        ("EGC", speeduino_data.ego_correction.to_string()),
        // WEC: Warm-Up Enrichment Correction
        ("WEC", speeduino_data.wue_correction.to_string()),
        // SCL: Secondary Load
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
