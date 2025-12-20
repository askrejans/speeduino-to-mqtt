use crate::config::AppConfig;
use crate::errors::{ParseError, Result};
use crate::mqtt_handler::{build_topic_path, MqttMessage};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, warn};

// Data validation constants (conservative ranges)
const RPM_MAX: u16 = 15000;
const TEMP_MIN: i16 = -40;
const TEMP_MAX: i16 = 200;
const MAP_MAX: u16 = 400; // kPa
const TPS_MAX: u8 = 100;  // %
const BATTERY_MIN: f32 = 8.0;  // V
const BATTERY_MAX: f32 = 18.0; // V
const PRESSURE_MAX: u16 = 1000; // kPa (fuel/oil pressure)

/// Represents the Speeduino ECU data structure.
#[derive(Debug)]
struct SpeeduinoData {
    secl: u8,                       // Counter for +1s
    status1: u8,                    // Status byte 1
    engine: u8,                     // Engine status
    dwell: u8,                      // Dwell time
    map_low: u8,                    // Low byte of MAP sensor reading
    map_high: u8,                   // High byte of MAP sensor reading
    mat: u8,                        // Manifold Air Temperature sensor reading
    coolant_adc: u8,                // Coolant Analog-to-Digital Conversion value
    bat_correction: u8,             // Battery correction
    battery_10: u8,                 // Battery voltage * 10
    o2_primary: u8,                 // Primary O2 sensor reading
    ego_correction: u8,             // EGO Correction
    iat_correction: u8,             // IAT Correction
    wue_correction: u8,             // Warm-Up Enrichment Correction
    rpm_low: u8,                    // Low byte of RPM
    rpm_high: u8,                   // High byte of RPM
    tae_amount: u8,                 // TAE Amount
    corrections: u8,                // Corrections
    ve: u8,                         // Volumetric Efficiency
    afr_target: u8,                 // AFR Target
    pw1_low: u8,                    // Low byte of Pulse Width 1
    pw1_high: u8,                   // High byte of Pulse Width 1
    tps_dot: u8,                    // Throttle Position Sensor change per second
    advance: u8,                    // Ignition Advance
    tps: u8,                        // Throttle Position Sensor reading
    loops_per_second_low: u8,       // Low byte of loops per second
    loops_per_second_high: u8,      // High byte of loops per second
    free_ram_low: u8,               // Low byte of free RAM
    free_ram_high: u8,              // High byte of free RAM
    boost_target: u8,               // Boost Target
    boost_duty: u8,                 // Boost Duty
    spark: u8,                      // Spark
    rpm_dot_low: u8,                // Low byte of RPM DOT (assuming signed integer)
    rpm_dot_high: u8,               // High byte of RPM DOT (assuming signed integer)
    ethanol_pct: u8,                // Ethanol Percentage
    flex_correction: u8,            // Flex Fuel Correction
    flex_ign_correction: u8,        // Flex Fuel Ignition Correction
    idle_load: u8,                  // Idle Load
    test_outputs: u8,               // Test Outputs
    o2_secondary: u8,               // Secondary O2 sensor reading
    baro: u8,                       // Barometric Pressure
    canin: [u8; 16],                // CAN Input values
    tps_adc: u8,                    // Throttle Position Sensor ADC value
    next_error: u8,                 // Next Error
    launch_correction: u8,          // Launch control correction
    pw2_low: u8,                    // Low byte of Pulse Width 2
    pw2_high: u8,                   // High byte of Pulse Width 2
    pw3_low: u8,                    // Low byte of Pulse Width 3
    pw3_high: u8,                   // High byte of Pulse Width 3
    pw4_low: u8,                    // Low byte of Pulse Width 4
    pw4_high: u8,                   // High byte of Pulse Width 4
    status3: u8,                    // Status3 bitfield
    engine_protect_status: u8,      // Engine protection status
    fuel_load_low: u8,              // Low byte of fuel load
    fuel_load_high: u8,             // High byte of fuel load
    ign_load_low: u8,               // Low byte of ignition load
    ign_load_high: u8,              // High byte of ignition load
    inj_angle_low: u8,              // Low byte of injection angle
    inj_angle_high: u8,             // High byte of injection angle
    idle_duty: u8,                  // Idle duty cycle
    cl_idle_target: u8,             // Closed loop idle target
    map_dot: u8,                    // MAP rate of change
    vvt1_angle: i8,                 // VVT1 angle
    vvt1_target_angle: u8,          // VVT1 target angle
    vvt1_duty: u8,                  // VVT1 duty cycle
    flex_boost_correction_low: u8,  // Low byte of flex boost correction
    flex_boost_correction_high: u8, // High byte of flex boost correction
    baro_correction: u8,            // Barometric pressure correction
    ase_value: u8,                  // Current ASE value
    vss_low: u8,                    // Low byte of vehicle speed
    vss_high: u8,                   // High byte of vehicle speed
    gear: u8,                       // Current gear
    fuel_pressure: u8,              // Fuel pressure
    oil_pressure: u8,               // Oil pressure
    wmi_pw: u8,                     // Water-methanol injection pulse width
    status4: u8,                    // Status4 bitfield
    vvt2_angle: i8,                 // VVT2 angle
    vvt2_target_angle: u8,          // VVT2 target angle
    vvt2_duty: u8,                  // VVT2 duty cycle
    outputs_status: u8,             // Outputs status
    fuel_temp: u8,                  // Fuel temperature
    fuel_temp_correction: u8,       // Fuel temperature correction
    ve1: u8,                        // VE table 1 value
    ve2: u8,                        // VE table 2 value
    advance1: u8,                   // Advance table 1 value
    advance2: u8,                   // Advance table 2 value
    nitrous_status: u8,             // Nitrous system status
    ts_sd_status: u8,               // SD card status
}

/// Process and publish the received Speeduino ECU data
///
/// # Arguments
///
/// * `data` - A slice of bytes representing received data.
/// * `config` - The Arc<AppConfig> instance.
/// * `mqtt_sender` - The MQTT message channel sender.
pub async fn process_speeduino_realtime_data(
    data: &[u8],
    config: &Arc<AppConfig>,
    mqtt_sender: &mpsc::Sender<MqttMessage>,
) -> Result<()> {
    // Validate data length
    if data.len() < config.expected_data_length {
        warn!("Invalid data received. Expected {} bytes, got {}",
            config.expected_data_length, data.len());
        return Err(ParseError::InsufficientData {
            expected: config.expected_data_length,
            actual: data.len(),
        }.into());
    }

    debug!("Parsing {} bytes of ECU data", data.len());

    // Parse the Realtime Data List
    let speeduino_data = parse_realtime_data(data)?;

    // Publish all parameters to MQTT
    publish_speeduino_params_to_mqtt(mqtt_sender, config, &speeduino_data).await?;

    Ok(())
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

/// Validates critical ECU data parameters against safe operating ranges.
/// 
/// Logs warnings for out-of-range values but doesn't fail parsing.
/// This allows the system to continue operating while alerting about suspicious data.
fn validate_data(data: &SpeeduinoData) -> Result<()> {
    // Validate RPM
    let rpm = combine_bytes(data.rpm_high, data.rpm_low);
    if rpm > RPM_MAX {
        warn!("RPM out of range: {} (max: {})", rpm, RPM_MAX);
    }
    
    // Validate coolant temperature (-40 to +200°C)
    let coolant_temp = data.coolant_adc as i16 - 40;
    if coolant_temp < TEMP_MIN || coolant_temp > TEMP_MAX {
        warn!("Coolant temp out of range: {}°C (range: {} to {})", 
              coolant_temp, TEMP_MIN, TEMP_MAX);
    }
    
    // Validate MAT temperature
    let mat_temp = data.mat as i16 - 40;
    if mat_temp < TEMP_MIN || mat_temp > TEMP_MAX {
        warn!("MAT temp out of range: {}°C (range: {} to {})", 
              mat_temp, TEMP_MIN, TEMP_MAX);
    }
    
    // Validate MAP sensor
    let map = combine_bytes(data.map_high, data.map_low);
    if map > MAP_MAX {
        warn!("MAP out of range: {} kPa (max: {})", map, MAP_MAX);
    }
    
    // Validate TPS
    if data.tps > TPS_MAX {
        warn!("TPS out of range: {}% (max: {})", data.tps, TPS_MAX);
    }
    
    // Validate battery voltage (allow 0V for disconnected battery)
    let battery_voltage = data.battery_10 as f32 / 10.0;
    if battery_voltage > 0.0 && (battery_voltage < BATTERY_MIN || battery_voltage > BATTERY_MAX) {
        warn!("Battery voltage out of range: {}V (range: {} to {})", 
              battery_voltage, BATTERY_MIN, BATTERY_MAX);
    }
    
    // Validate fuel pressure (single byte, 0-255 kPa range)
    if data.fuel_pressure as u16 > PRESSURE_MAX {
        warn!("Fuel pressure out of range: {} kPa (max: {})", data.fuel_pressure, PRESSURE_MAX);
    }
    
    // Validate oil pressure (single byte, 0-255 kPa range)
    if data.oil_pressure as u16 > PRESSURE_MAX {
        warn!("Oil pressure out of range: {} kPa (max: {})", data.oil_pressure, PRESSURE_MAX);
    }
    
    // Validate fuel temperature
    let fuel_temp = data.fuel_temp as i16 - 40;
    if fuel_temp < TEMP_MIN || fuel_temp > TEMP_MAX {
        warn!("Fuel temp out of range: {}°C (range: {} to {})", 
              fuel_temp, TEMP_MIN, TEMP_MAX);
    }
    
    debug!("Data validation passed");
    Ok(())
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
fn parse_realtime_data(data: &[u8]) -> Result<SpeeduinoData> {
    let mut offset = 0;

    fn read_byte(data: &[u8], offset: &mut usize) -> Result<u8> {
        if *offset < data.len() {
            let value = data[*offset];
            *offset += 1;
            Ok(value)
        } else {
            Err(ParseError::InsufficientData {
                expected: *offset + 1,
                actual: data.len(),
            }.into())
        }
    }

    // Create a SpeeduinoData instance by reading each field
    let speeduino_data = SpeeduinoData {
        secl: read_byte(data, &mut offset)?,
        status1: read_byte(data, &mut offset)?,
        engine: read_byte(data, &mut offset)?,
        dwell: read_byte(data, &mut offset)?,
        map_low: read_byte(data, &mut offset)?,
        map_high: read_byte(data, &mut offset)?,
        mat: read_byte(data, &mut offset)?,
        coolant_adc: read_byte(data, &mut offset)?,
        bat_correction: read_byte(data, &mut offset)?,
        battery_10: read_byte(data, &mut offset)?,
        o2_primary: read_byte(data, &mut offset)?,
        ego_correction: read_byte(data, &mut offset)?,
        iat_correction: read_byte(data, &mut offset)?,
        wue_correction: read_byte(data, &mut offset)?,
        rpm_low: read_byte(data, &mut offset)?,
        rpm_high: read_byte(data, &mut offset)?,
        tae_amount: read_byte(data, &mut offset)?,
        corrections: read_byte(data, &mut offset)?,
        ve: read_byte(data, &mut offset)?,
        afr_target: read_byte(data, &mut offset)?,
        pw1_low: read_byte(data, &mut offset)?,
        pw1_high: read_byte(data, &mut offset)?,
        tps_dot: read_byte(data, &mut offset)?,
        advance: read_byte(data, &mut offset)?,
        tps: read_byte(data, &mut offset)?,
        loops_per_second_low: read_byte(data, &mut offset)?,
        loops_per_second_high: read_byte(data, &mut offset)?,
        free_ram_low: read_byte(data, &mut offset)?,
        free_ram_high: read_byte(data, &mut offset)?,
        boost_target: read_byte(data, &mut offset)?,
        boost_duty: read_byte(data, &mut offset)?,
        spark: read_byte(data, &mut offset)?,
        rpm_dot_low: read_byte(data, &mut offset)?,
        rpm_dot_high: read_byte(data, &mut offset)?,
        ethanol_pct: read_byte(data, &mut offset)?,
        flex_correction: read_byte(data, &mut offset)?,
        flex_ign_correction: read_byte(data, &mut offset)?,
        idle_load: read_byte(data, &mut offset)?,
        test_outputs: read_byte(data, &mut offset)?,
        o2_secondary: read_byte(data, &mut offset)?,
        baro: read_byte(data, &mut offset)?,
        canin: [
            read_byte(data, &mut offset)?,
            read_byte(data, &mut offset)?,
            read_byte(data, &mut offset)?,
            read_byte(data, &mut offset)?,
            read_byte(data, &mut offset)?,
            read_byte(data, &mut offset)?,
            read_byte(data, &mut offset)?,
            read_byte(data, &mut offset)?,
            read_byte(data, &mut offset)?,
            read_byte(data, &mut offset)?,
            read_byte(data, &mut offset)?,
            read_byte(data, &mut offset)?,
            read_byte(data, &mut offset)?,
            read_byte(data, &mut offset)?,
            read_byte(data, &mut offset)?,
            read_byte(data, &mut offset)?,
        ],
        tps_adc: read_byte(data, &mut offset)?,
        next_error: read_byte(data, &mut offset)?,
        launch_correction: read_byte(data, &mut offset)?,
        pw2_low: read_byte(data, &mut offset)?,
        pw2_high: read_byte(data, &mut offset)?,
        pw3_low: read_byte(data, &mut offset)?,
        pw3_high: read_byte(data, &mut offset)?,
        pw4_low: read_byte(data, &mut offset)?,
        pw4_high: read_byte(data, &mut offset)?,
        status3: read_byte(data, &mut offset)?,
        engine_protect_status: read_byte(data, &mut offset)?,
        fuel_load_low: read_byte(data, &mut offset)?,
        fuel_load_high: read_byte(data, &mut offset)?,
        ign_load_low: read_byte(data, &mut offset)?,
        ign_load_high: read_byte(data, &mut offset)?,
        inj_angle_low: read_byte(data, &mut offset)?,
        inj_angle_high: read_byte(data, &mut offset)?,
        idle_duty: read_byte(data, &mut offset)?,
        cl_idle_target: read_byte(data, &mut offset)?,
        map_dot: read_byte(data, &mut offset)?,
        vvt1_angle: read_byte(data, &mut offset)? as i8,
        vvt1_target_angle: read_byte(data, &mut offset)?,
        vvt1_duty: read_byte(data, &mut offset)?,
        flex_boost_correction_low: read_byte(data, &mut offset)?,
        flex_boost_correction_high: read_byte(data, &mut offset)?,
        baro_correction: read_byte(data, &mut offset)?,
        ase_value: read_byte(data, &mut offset)?,
        vss_low: read_byte(data, &mut offset)?,
        vss_high: read_byte(data, &mut offset)?,
        gear: read_byte(data, &mut offset)?,
        fuel_pressure: read_byte(data, &mut offset)?,
        oil_pressure: read_byte(data, &mut offset)?,
        wmi_pw: read_byte(data, &mut offset)?,
        status4: read_byte(data, &mut offset)?,
        vvt2_angle: read_byte(data, &mut offset)? as i8,
        vvt2_target_angle: read_byte(data, &mut offset)?,
        vvt2_duty: read_byte(data, &mut offset)?,
        outputs_status: read_byte(data, &mut offset)?,
        fuel_temp: read_byte(data, &mut offset)?,
        fuel_temp_correction: read_byte(data, &mut offset)?,
        ve1: read_byte(data, &mut offset)?,
        ve2: read_byte(data, &mut offset)?,
        advance1: read_byte(data, &mut offset)?,
        advance2: read_byte(data, &mut offset)?,
        nitrous_status: read_byte(data, &mut offset)?,
        ts_sd_status: read_byte(data, &mut offset)?,
    };

    // Validate critical parameters
    validate_data(&speeduino_data)?;
    
    Ok(speeduino_data)
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
        ("LNC", speeduino_data.launch_correction.to_string()),
        (
            "PW2",
            combine_bytes(speeduino_data.pw2_high, speeduino_data.pw2_low).to_string(),
        ),
        (
            "PW3",
            combine_bytes(speeduino_data.pw3_high, speeduino_data.pw3_low).to_string(),
        ),
        (
            "PW4",
            combine_bytes(speeduino_data.pw4_high, speeduino_data.pw4_low).to_string(),
        ),
        ("ST3", speeduino_data.status3.to_string()),
        ("EPS", speeduino_data.engine_protect_status.to_string()),
        (
            "FLD",
            combine_bytes(speeduino_data.fuel_load_high, speeduino_data.fuel_load_low).to_string(),
        ),
        (
            "IGD",
            combine_bytes(speeduino_data.ign_load_high, speeduino_data.ign_load_low).to_string(),
        ),
        (
            "INA",
            combine_bytes(speeduino_data.inj_angle_high, speeduino_data.inj_angle_low).to_string(),
        ),
        ("IDY", speeduino_data.idle_duty.to_string()),
        ("CLT", speeduino_data.cl_idle_target.to_string()),
        ("MPD", speeduino_data.map_dot.to_string()),
        ("VA1", speeduino_data.vvt1_angle.to_string()),
        ("VT1", speeduino_data.vvt1_target_angle.to_string()),
        ("VD1", speeduino_data.vvt1_duty.to_string()),
        (
            "FBC",
            combine_bytes(
                speeduino_data.flex_boost_correction_high,
                speeduino_data.flex_boost_correction_low,
            )
            .to_string(),
        ),
        ("BRC", speeduino_data.baro_correction.to_string()),
        ("ASE", speeduino_data.ase_value.to_string()),
        (
            "VSS",
            combine_bytes(speeduino_data.vss_high, speeduino_data.vss_low).to_string(),
        ),
        ("GER", speeduino_data.gear.to_string()),
        ("FPR", speeduino_data.fuel_pressure.to_string()),
        ("OPR", speeduino_data.oil_pressure.to_string()),
        ("WMI", speeduino_data.wmi_pw.to_string()),
        ("ST4", speeduino_data.status4.to_string()),
        ("VA2", speeduino_data.vvt2_angle.to_string()),
        ("VT2", speeduino_data.vvt2_target_angle.to_string()),
        ("VD2", speeduino_data.vvt2_duty.to_string()),
        ("OUT", speeduino_data.outputs_status.to_string()),
        ("FTP", (speeduino_data.fuel_temp as i16 - 40).to_string()), // Apply temperature offset
        ("FTC", speeduino_data.fuel_temp_correction.to_string()),
        ("VE1", speeduino_data.ve1.to_string()),
        ("VE2", speeduino_data.ve2.to_string()),
        ("AD1", speeduino_data.advance1.to_string()),
        ("AD2", speeduino_data.advance2.to_string()),
        ("NOS", speeduino_data.nitrous_status.to_string()),
        ("SDS", speeduino_data.ts_sd_status.to_string()),
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
async fn publish_speeduino_params_to_mqtt(
    mqtt_sender: &mpsc::Sender<MqttMessage>,
    config: &Arc<AppConfig>,
    speeduino_data: &SpeeduinoData,
) -> Result<()> {
    let params_to_publish = get_params_to_publish(speeduino_data);

    for (param_code, param_value) in params_to_publish {
        publish_param_to_mqtt(mqtt_sender, config, param_code, param_value).await?;
    }

    Ok(())
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
async fn publish_param_to_mqtt(
    mqtt_sender: &mpsc::Sender<MqttMessage>,
    config: &Arc<AppConfig>,
    param_code: &str,
    param_value: String,
) -> Result<()> {
    let topic = build_topic_path(&config.mqtt_base_topic, param_code);
    
    let message = MqttMessage::new(topic, param_value, config.mqtt_qos);
    
    mqtt_sender
        .send(message)
        .await
        .map_err(|_| ParseError::InvalidData {
            offset: 0,
            message: "Failed to queue MQTT message".to_string(),
        })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_realtime_data_valid() {
        let data: [u8; 120] = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
            0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C,
            0x1D, 0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A,
            0x2B, 0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38,
            0x39, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E, 0x3F, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46,
            0x47, 0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F, 0x50, 0x51, 0x52, 0x53, 0x54,
            0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F, 0x60, 0x61, 0x62,
            0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6A, 0x6B, 0x6C, 0x6D, 0x6E, 0x6F, 0x70,
            0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78,
        ];

        let result = parse_realtime_data(&data).expect("Should parse successfully");

        // Assert that first fields are correctly parsed
        assert_eq!(result.secl, 0x01);
        assert_eq!(result.status1, 0x02);
        assert_eq!(result.engine, 0x03);
        assert_eq!(result.dwell, 0x04);
        assert_eq!(result.map_low, 0x05);
        assert_eq!(result.map_high, 0x06);
    }

    #[test]
    fn test_parse_realtime_data_insufficient() {
        let data: [u8; 50] = [0; 50];
        let result = parse_realtime_data(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_realtime_data_empty() {
        let data: [u8; 0] = [];
        let result = parse_realtime_data(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_combine_bytes() {
        assert_eq!(combine_bytes(0x12, 0x34), 0x1234);
        assert_eq!(combine_bytes(0xFF, 0xFF), 0xFFFF);
        assert_eq!(combine_bytes(0x00, 0x00), 0x0000);
        assert_eq!(combine_bytes(0xAB, 0xCD), 0xABCD);
    }

    #[test]
    fn test_validate_data_normal_values() {
        let mut data = create_test_data();
        // Set normal values
        data.rpm_high = 0x0B;  // RPM = 3000
        data.rpm_low = 0xB8;
        data.coolant_adc = 100; // 60°C
        data.mat = 80;          // 40°C
        data.tps = 50;          // 50%
        data.battery_10 = 140;  // 14.0V
        
        let result = validate_data(&data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_data_rpm_too_high() {
        let mut data = create_test_data();
        data.rpm_high = 0xFF;  // Very high RPM
        data.rpm_low = 0xFF;
        
        // Should still succeed but log a warning
        let result = validate_data(&data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_data_temp_out_of_range() {
        let mut data = create_test_data();
        data.coolant_adc = 255; // 215°C (out of range)
        
        // Should still succeed but log a warning
        let result = validate_data(&data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_data_battery_low() {
        let mut data = create_test_data();
        data.battery_10 = 70; // 7.0V (too low)
        
        // Should still succeed but log a warning
        let result = validate_data(&data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_data_battery_zero_allowed() {
        let mut data = create_test_data();
        data.battery_10 = 0; // 0.0V (disconnected battery - should be allowed)
        
        // Should succeed without warning
        let result = validate_data(&data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_data_tps_over_100() {
        let mut data = create_test_data();
        data.tps = 150; // > 100%
        
        // Should still succeed but log a warning
        let result = validate_data(&data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_params_to_publish_count() {
        let data = create_test_data();
        let params = get_params_to_publish(&data);
        
        // Should have all 81 parameters (originally 73, expanded with new fields)
        assert_eq!(params.len(), 81);
    }

    #[test]
    fn test_get_params_to_publish_rpm() {
        let mut data = create_test_data();
        data.rpm_high = 0x0B;
        data.rpm_low = 0xB8;
        
        let params = get_params_to_publish(&data);
        let rpm_param = params.iter().find(|(code, _)| *code == "RPM");
        
        assert!(rpm_param.is_some());
        assert_eq!(rpm_param.unwrap().1, "3000");
    }

    #[test]
    fn test_get_params_to_publish_tps() {
        let mut data = create_test_data();
        data.tps = 75;
        
        let params = get_params_to_publish(&data);
        let tps_param = params.iter().find(|(code, _)| *code == "TPS");
        
        assert!(tps_param.is_some());
        assert_eq!(tps_param.unwrap().1, "75");
    }

    #[test]
    fn test_get_params_to_publish_battery() {
        let mut data = create_test_data();
        data.battery_10 = 140; // 14.0V
        
        let params = get_params_to_publish(&data);
        let bat_param = params.iter().find(|(code, _)| *code == "BAT");
        
        assert!(bat_param.is_some());
        assert_eq!(bat_param.unwrap().1, "14");
    }

    /// Helper function to create test data with default values
    fn create_test_data() -> SpeeduinoData {
        SpeeduinoData {
            secl: 0,
            status1: 0,
            engine: 0,
            dwell: 0,
            map_low: 0,
            map_high: 0,
            mat: 80,
            coolant_adc: 100,
            bat_correction: 100,
            battery_10: 140,
            o2_primary: 0,
            ego_correction: 100,
            iat_correction: 100,
            wue_correction: 100,
            rpm_low: 0,
            rpm_high: 0,
            tae_amount: 0,
            corrections: 100,
            ve: 100,
            afr_target: 147,
            pw1_low: 0,
            pw1_high: 0,
            tps_dot: 0,
            advance: 0,
            tps: 0,
            loops_per_second_low: 0,
            loops_per_second_high: 0,
            free_ram_low: 0,
            free_ram_high: 0,
            boost_target: 0,
            boost_duty: 0,
            spark: 0,
            rpm_dot_low: 0,
            rpm_dot_high: 0,
            ethanol_pct: 0,
            flex_correction: 0,
            flex_ign_correction: 0,
            idle_load: 0,
            test_outputs: 0,
            o2_secondary: 0,
            baro: 100,
            canin: [0; 16],
            tps_adc: 0,
            next_error: 0,
            launch_correction: 0,
            pw2_low: 0,
            pw2_high: 0,
            pw3_low: 0,
            pw3_high: 0,
            pw4_low: 0,
            pw4_high: 0,
            status3: 0,
            engine_protect_status: 0,
            fuel_load_low: 0,
            fuel_load_high: 0,
            ign_load_low: 0,
            ign_load_high: 0,
            inj_angle_low: 0,
            inj_angle_high: 0,
            idle_duty: 0,
            cl_idle_target: 0,
            map_dot: 0,
            vvt1_angle: 0,
            vvt1_target_angle: 0,
            vvt1_duty: 0,
            flex_boost_correction_low: 0,
            flex_boost_correction_high: 0,
            baro_correction: 100,
            ase_value: 0,
            vss_low: 0,
            vss_high: 0,
            gear: 0,
            fuel_pressure: 200,
            oil_pressure: 150,
            wmi_pw: 0,
            status4: 0,
            vvt2_angle: 0,
            vvt2_target_angle: 0,
            vvt2_duty: 0,
            outputs_status: 0,
            fuel_temp: 60,
            fuel_temp_correction: 100,
            ve1: 100,
            ve2: 100,
            advance1: 0,
            advance2: 0,
            nitrous_status: 0,
            ts_sd_status: 0,
        }
    }
}

