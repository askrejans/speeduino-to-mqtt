# Speeduino-to-MQTT

A Rust application that reads real-time engine data from a [Speeduino](https://speeduino.com) ECU and publishes it to an MQTT broker. Supports hardware serial ports and TCP/IP bridges (WiFi, Ethernet), an interactive terminal UI for standalone/bench use, and fully optional MQTT so the app can run display-only without any broker.

> **Testing:** [speeduino-serial-sim](https://github.com/askrejans/speeduino-serial-sim) can be used to generate synthetic ECU data without a real ECU.

## Features

- **ECU protocol** – issues the [`A` real-time data command](https://wiki.speeduino.com/en/reference/Interface_Protocol), parses all bytes of the response including EMAP, CAN inputs (CN01–CN16), VVT, flex fuel, boost and more.
- **Dual connection modes** – hardware serial (`/dev/ttyACM0`, COM3 …) or raw TCP socket for WiFi/Ethernet–serial bridges (ESP32, Moxa, USR-VIS410, …).
- **Interactive TUI** – when run from a terminal (TTY detected) a live four-panel dashboard is displayed: connection status, ECU gauges, live MQTT stats and a scrolling log.
- **Optional MQTT** – set `mqtt_enabled = false` (or `SPEEDUINO_MQTT_ENABLED=false`) to run in display-only mode with no broker required.
- **Flexible configuration** – TOML config file, environment variables with `SPEEDUINO_` prefix, and automatic `.env` file loading from the working directory.
- **Systemd service** – ships with a ready-made service unit; the `scripts/build_packages.sh` helper builds installable DEB and RPM packages.
- **85+ MQTT topics** – every ECU parameter is published as a short three-letter code under a configurable base topic.

## Running modes

| Invocation | Behaviour |
|---|---|
| Terminal / bench (`ssh`, local shell) | Interactive TUI rendered via `ratatui` |
| `systemd` service / no TTY | Structured text logging to stdout |
| `mqtt_enabled = false` | No broker needed; data shown in TUI only |
| `mqtt_enabled = true` (default) | Data published to MQTT broker |

## Quick start

```bash
# 1 – Build
cargo build --release

# 2 – Copy and edit config
cp example.settings.toml settings.toml
$EDITOR settings.toml

# 3 – Run (TUI auto-enabled when attached to a terminal)
./target/release/speeduino-to-mqtt

# 4 – Or pass a custom config path
./target/release/speeduino-to-mqtt --config /etc/speeduino-to-mqtt/settings.toml
```

## CLI options

```
Usage: speeduino-to-mqtt [options]

Options:
  -h, --help          Print help
  -c, --config FILE   Path to TOML config file (default: settings.toml)
```

## Configuration

Copy `example.settings.toml` to `settings.toml` and adjust the values. Every setting can also be set via an environment variable with the `SPEEDUINO_` prefix, or in a `.env` file in the working directory.

```toml
# ── Connection ──────────────────────────────────────────────────
connection_type = "serial"   # "serial" | "tcp"

# Serial (used when connection_type = "serial")
port_name  = "/dev/ttyACM0"
baud_rate  = 115200

# TCP bridge (used when connection_type = "tcp")
# tcp_host = "192.168.1.100"
# tcp_port = 23

# ── ECU protocol ────────────────────────────────────────────────
# expected_data_length = 120   # 119–256; 121 enables EMAP
# read_timeout_ms      = 2000
refresh_rate_ms        = 20

# ── MQTT ────────────────────────────────────────────────────────
mqtt_enabled    = true
mqtt_host       = "localhost"
mqtt_port       = 1883
mqtt_base_topic = "/GOLF86/ECU/"
# mqtt_username = ""
# mqtt_password = ""
# mqtt_use_tls  = false
```

Key environment variables:

| Variable | Description |
|---|---|
| `SPEEDUINO_CONNECTION_TYPE` | `serial` or `tcp` |
| `SPEEDUINO_PORT_NAME` | Serial device path |
| `SPEEDUINO_TCP_HOST` / `SPEEDUINO_TCP_PORT` | TCP bridge address |
| `SPEEDUINO_MQTT_ENABLED` | `true` / `false` |
| `SPEEDUINO_MQTT_HOST` / `SPEEDUINO_MQTT_PORT` | Broker address |
| `SPEEDUINO_MQTT_USERNAME` / `SPEEDUINO_MQTT_PASSWORD` | Broker credentials |
| `SPEEDUINO_LOG_LEVEL` | `trace` \| `debug` \| `info` \| `warn` \| `error` |

## MQTT topics

All values are published to `<mqtt_base_topic><CODE>`, e.g. `/GOLF86/ECU/RPM`.

### Engine basics
| Code | Description |
|---|---|
| `RPM` | Engine speed (rev/min) |
| `TPS` | Throttle position (0–255 raw) |
| `MAP` | Manifold absolute pressure (kPa) |
| `BAR` | Barometric pressure (kPa) |
| `BAT` | Battery voltage (V, 1 dp) |
| `SCL` | Loop counter (secl) |
| `SYN` | Sync loss counter |

### Temperatures
| Code | Description |
|---|---|
| `IAT` | Intake air temperature (°C) |
| `CLT` | Coolant temperature (°C) |
| `MAT` | IAT raw byte (backward-compatible) |
| `CAD` | Coolant raw byte (backward-compatible) |
| `FTP` | Fuel temperature (°C) |

### O2 / AFR
| Code | Description |
|---|---|
| `O2P` | Primary O2 sensor |
| `O2S` | Secondary O2 sensor |
| `AFT` | AFR target (real units, 1 dp) |

### Fuel & injection
| Code | Description |
|---|---|
| `VE1` / `VE2` / `VEC` | Volumetric efficiency current / table 1 / table 2 |
| `PW1`–`PW4` | Injector pulse width channels 1–4 (ms, 1 dp) |
| `FLD` | Fuel load |
| `FTC` | Fuel temp correction |

### Ignition
| Code | Description |
|---|---|
| `ADV` / `AD1` / `AD2` | Ignition advance (degrees) |
| `DWL` | Dwell time (ms, 1 dp) |
| `SPK` | Spark status bitfield |
| `IGD` | Ignition load |

### Corrections
| Code | Description |
|---|---|
| `COR` | Combined corrections |
| `BTC` | Battery correction |
| `EGC` | EGO (O2) correction |
| `ITC` | IAT correction |
| `WEC` | Warm-up enrichment correction |
| `BRC` | Baro correction |
| `ASE` | After-start enrichment |
| `TAE` | Transient acceleration enrichment (%) |

### Flex fuel / ethanol
| Code | Description |
|---|---|
| `ETH` | Ethanol % |
| `FLC` | Flex fuel correction |
| `FIC` | Flex ignition correction |
| `FBC` | Flex boost correction |

### Boost
| Code | Description |
|---|---|
| `BST` | Boost target (kPa) |
| `BSD` | Boost duty cycle (%) |

### VVT
| Code | Description |
|---|---|
| `VA1` / `VA2` | VVT 1/2 actual angle |
| `VT1` / `VT2` | VVT 1/2 target angle |
| `VD1` / `VD2` | VVT 1/2 duty cycle |

### CAN inputs
| Code | Description |
|---|---|
| `CN01`–`CN16` | CAN input channels 1–16 (u16 each) |

### Miscellaneous
| Code | Description |
|---|---|
| `VSS` | Vehicle speed |
| `GER` | Current gear |
| `FPR` | Fuel pressure |
| `OPR` | Oil pressure |
| `ILL` | Idle load |
| `MPD` | MAP dot (rate of change) |
| `TPD` | TPS dot |
| `TAD` | TPS ADC |
| `CIT` | Closed-loop idle target |
| `WMI` | WMI pulse width |
| `LPS` | Loops per second |
| `FRM` | Free RAM |
| `RPD` | RPM dot |
| `TOF` | Test output flags |
| `NER` | Next error code |
| `STA` / `ENG` / `ST3` / `ST4` | Status bitfields |
| `EPS` | Engine protect status |
| `OUT` | Output status |
| `SDS` | SD card / TunerStudio status |
| `EMP` | EMAP pressure (published only when packet ≥ 121 bytes) |

## Building packages (DEB / RPM)

The `scripts/build_packages.sh` script cross-compiles the binary and assembles installable packages for **x86-64** and **arm64** using `cross`, `dpkg-deb`, and `rpmbuild`.

```bash
# Build DEB + RPM for both architectures (requires cross, dpkg-dev, rpm-build)
./scripts/build_packages.sh

# Single architecture / format
./scripts/build_packages.sh --arch arm64 --type deb

# Use local cargo toolchain instead of cross
./scripts/build_packages.sh --no-cross

# Help
./scripts/build_packages.sh --help
```

Packages are written to `release/<version>/deb/` and `release/<version>/rpm/`.

Each package:
- Installs the binary to `/usr/bin/speeduino-to-mqtt`
- Installs the systemd unit to `/lib/systemd/system/speeduino-to-mqtt.service`
- Places an example config in `/etc/speeduino-to-mqtt/settings.toml.example`
- Enables the service via `systemctl enable` in the post-install script

After installation:

```bash
sudo cp /etc/speeduino-to-mqtt/settings.toml.example /etc/speeduino-to-mqtt/settings.toml
sudo $EDITOR /etc/speeduino-to-mqtt/settings.toml
sudo systemctl start speeduino-to-mqtt
```

## Related projects

- [speeduino-serial-sim](https://github.com/askrejans/speeduino-serial-sim) – ECU data simulator for testing
- [GPS-to-MQTT](https://github.com/askrejans/gps-to-mqtt) – companion GPS bridge
- [G86 Web Dashboard](https://github.com/askrejans/G86-web-dashboard) – web dashboard for MQTT telemetry data

