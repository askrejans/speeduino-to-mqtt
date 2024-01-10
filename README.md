# Speeduino-to-MQTT

Speeduino-to-MQTT is a Rust application designed to read Speeduino Engine Control Unit (ECU) serial communications and transmit essential engine data to an MQTT broker. This application utilizes the "A" command for real-time data retrieval from the ECU and processes the returned results.

**Notice:**
This software is currently in the early stages of development. Use it at your own risk. It has undergone limited testing with a 1000ms sample rate on a single device and may not be suitable for high-speed communications. Further testing is advised.

## Features

- Reads Speeduino ECU serial communications, sends the "A" [command](https://wiki.speeduino.com/en/reference/Interface_Protocol), and parses the engine data returned.
- Transmits parsed basic engine data to an MQTT broker.

## Usage

1. Download the latest release from the [releases page](https://github.com/your-username/speeduino-to-mqtt).

2. Edit the `settings.toml` file with your configurations. An example `settings.toml` is included in the root directory.

3. Build the application by setting up the Rust build environment and executing `cargo build --release`.

4. Copy the `settings.toml` file next to the executable at `target/release/speeduino-to-mqtt` and run the application.

