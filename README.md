# Speeduino-to-MQTT

Speeduino-to-MQTT is a Rust app built for reading Speeduino Engine Control Unit (ECU) serial signals and beaming crucial engine data to an MQTT broker. It gets the serial data using the "A" command for real-time ECU data. For testing purpouses [speeduino-serial-sim](https://github.com/askrejans/speeduino-serial-sim) can be used to generate test data.

**Note:**
This software is in early development, so use it at your own risk. It's been tested successfully only with a 1000ms sample rate on a single device, but it might struggle with high-speed stuff, so more testing is needed here.

## Features

- Hooks into Speeduino ECU serial signals, hits it with the ["A" command](https://wiki.speeduino.com/en/reference/Interface_Protocol), and parses out the engine data.
- Pushes parsed engine data onto an MQTT broker. Right now, no encrypted connections or logins because to keep it simple for car LAN use.

## How to Use

1. **Grab the Latest Version**

2. **Set it Up:** Tweak the `settings.toml` file to match your setup. There's a sample `example.settings.toml` in the main directory.

3. **Get it Running:** Set up your Rust build environment and fire up the build with:

    ```bash
    cargo build --release
    ```

4. **Go Live:** Stick the `settings.toml` next to the executable at `target/release/speeduino-to-mqtt` and kick off the app.

   ```bash
   ./target/release/speeduino-to-mqtt

## MQTT data example:
Data is pushed as 3 letter codes (explained in source) to a configured MQTT topic.
```code
RPM: Engine revolutions per minute
TPS: Throttle Position Sensor reading (0% to 100%)
VE1: Volumetric Efficiency (%)
O2P: Primary O2 sensor reading
AFT: Air-Fuel Ratio Target
MAT: Manifold Air Temperature sensor reading
CAD: Coolant Analog-to-Digital Conversion value
MAP: Manifold Absolute Pressure sensor reading
BAT: Battery voltage (scaled by 10)
ADV: Ignition Advance
PW1: Pulse Width 1
SPK: Spark
DWL: Dwell time
ILL: Idle Load
BAR: Barometric Pressure
TAE: Warm-Up Enrichment Correction (%)
NER: Next Error code
ENG: Engine status
```
More info:  [wiki.speeduino.com](https://wiki.speeduino.com/en/reference/Interface_Protocol)

![image](https://github.com/askrejans/speeduino-to-mqtt/assets/1042303/d1b8cbd3-3f9a-471b-8dc1-bf206eb39693)

## Pre-Built Packages

There are also pre build packages, that combines three individual components: [Speeduino-to-MQTT](https://github.com/askrejans/speeduino-to-mqtt), [GPS-to-MQTT](https://github.com/askrejans/gps-to-mqtt), and [G86 Web Dashboard](https://github.com/askrejans/G86-web-dashboard) in one system with predefined services.

You can quickly get started by using pre-built packages available for both x64 and Raspberry Pi 4 (ARM) architectures:

- **DEB Packages for x64:** [Download here](https://akelaops.com/repo/deb/pool/main/amd64/g86-car-telemetry_1.0.deb)
- **DEB Packages for Raspberry Pi 4 (ARM):** [Download here](https://akelaops.com/repo/deb/pool/main/aarch64/g86-car-telemetry_1.0.deb)
- **RPM Packages for x64:** [Download here](https://akelaops.com/repo/rpm/x86_64/g86-car-telemetry-1.0-1.x86_64.rpm)
- **RPM Packages for Raspberry Pi 4 (ARM):** [Download here](https://akelaops.com/repo/rpm/aarch64/g86-car-telemetry-1.0-1.aarch64.rpm)

### Package Installation Details

- All packages install the three services in the directory `/opt/g86-car-telemetry` (or `/usr/opt/g86-car-telemetry`).
- Configuration files for GPS and ECU processors can be found under `/etc/g86-car-telemetry` (or `/usr/etc/g86-car-telemetry`).
- Web project configurations are located in `/var/www/g86-car-telemetry/config` (or `/usr/var/www/g86-car-telemetry/config`).
- Ensure to add relevant configurations for MQTT server, TTY ports, and any extra settings.

### Installed Services

The packages automatically install and manage the following services:

- `g86-car-telemetry-gps`
- `g86-car-telemetry-speeduino`
- `g86-car-telemetry-web`

### Compatibility and Testing

These packages have been tested on both Raspberry Pi 4 (ARM) with DEB packages and x86 systems with RPM packages. However, please note that this project is a work in progress, and more tests are needed, especially with real ECUs. Exercise caution when using, and stay tuned for updates as development continues to enhance and stabilize the functionality.

Feel free to reach out if you have any questions or encounter issues. Happy telemetry monitoring! 📊🛠️



