//! ECU Communication Handler
//!
//! Handles async communication with the Speeduino ECU over either a hardware
//! serial port or a raw TCP socket (e.g. an ESP32 / Moxa WiFi bridge).
//! Implements exponential backoff for reconnection and robust error handling.

use crate::config::AppConfig;
use crate::connection::EcuConnection;
use crate::errors::{Result, SerialError};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, warn};

/// ECU command to request realtime data
const ECU_COMMAND: u8 = b'A';

/// Minimum bytes we must receive to have a valid primary-serial packet.
const MIN_PACKET_BYTES: usize = 130;
/// Maximum buffer size — larger than any known Speeduino packet.
const MAX_PACKET_BYTES: usize = 512;
/// Silence (ms) we wait before sending the command, to discard stale bytes.
const PRE_DRAIN_IDLE_MS: u64 = 20;
/// Silence (ms) after which we consider the response complete.
/// Only applied once MIN_PACKET_BYTES have been received.
/// 30 ms >> 138-byte transmission time (~12 ms at 115200 baud).
const IDLE_DONE_MS: u64 = 30;

/// ECU connection handler — supports hardware serial and TCP (e.g. WiFi bridge).
pub struct EcuSerialHandler {
    connection: Option<EcuConnection>,
    config: AppConfig,
    retry_count: u32,
    current_delay_ms: u64,
}

impl EcuSerialHandler {
    /// Create a new ECU serial handler
    pub fn new(config: AppConfig) -> Self {
        Self {
            connection: None,
            config: config.clone(),
            retry_count: 0,
            current_delay_ms: config.initial_retry_delay_ms,
        }
    }

    /// Open the ECU connection (serial or TCP depending on config).
    pub async fn connect(&mut self) -> Result<()> {
        info!("Connecting to ECU: {}", self.config.connection_display());

        let conn = EcuConnection::open(&self.config).await?;
        self.connection = Some(conn);
        self.retry_count = 0;
        self.current_delay_ms = self.config.initial_retry_delay_ms;

        info!("Successfully connected to ECU");
        Ok(())
    }

    /// Returns `true` when an active connection exists.
    #[allow(dead_code)]
    pub fn is_connected(&self) -> bool {
        self.connection.is_some()
    }

    /// Close the ECU connection.
    pub async fn disconnect(&mut self) {
        if self.connection.take().is_some() {
            debug!("ECU connection closed");
        }
    }

    /// Read engine data from the ECU.
    ///
    /// Strategy (no sleep, no kernel buffer flush — safe on Linux USB-serial):
    ///   1. Pre-drain: discard bytes until PRE_DRAIN_IDLE_MS of silence.
    ///   2. Send `A` + flush.
    ///   3. Collect: keep reading.  Once MIN_PACKET_BYTES have arrived,
    ///      switch to IDLE_DONE_MS idle timeout to detect end-of-packet.
    ///
    /// Short or garbled packets are returned as a soft error — the caller
    /// discards them and tries again next poll cycle without reconnecting.
    pub async fn read_engine_data(&mut self) -> Result<Vec<u8>> {
        let conn = self.connection.as_mut().ok_or(SerialError::Disconnected)?;

        // ── 1. Pre-drain ────────────────────────────────────────────────────
        Self::drain_until_quiet(conn, PRE_DRAIN_IDLE_MS).await;

        // ── 2. Send command ─────────────────────────────────────────────────
        debug!("Sending ECU command: 0x{:02X}", ECU_COMMAND);
        conn.write_all(&[ECU_COMMAND])
            .await
            .map_err(SerialError::WriteFailed)?;
        conn.flush().await.map_err(SerialError::WriteFailed)?;

        // ── 3. Collect response ──────────────────────────────────────────────
        let deadline = tokio::time::Instant::now()
            + Duration::from_millis(self.config.read_timeout_ms);
        let mut buf: Vec<u8> = Vec::with_capacity(MAX_PACKET_BYTES);
        let mut tmp = [0u8; 64];

        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            let wait = if buf.len() >= MIN_PACKET_BYTES {
                remaining.min(Duration::from_millis(IDLE_DONE_MS))
            } else {
                remaining
            };
            match timeout(wait, conn.read(&mut tmp)).await {
                Ok(Ok(0)) | Err(_) => break,
                Ok(Ok(n)) => {
                    buf.extend_from_slice(&tmp[..n]);
                    if buf.len() >= MAX_PACKET_BYTES {
                        break;
                    }
                }
                Ok(Err(e)) => return Err(SerialError::ReadFailed(e).into()),
            }
        }

        if buf.len() < MIN_PACKET_BYTES {
            debug!("Short packet: {} bytes — discarding", buf.len());
            return Err(SerialError::ReadTimeout {
                timeout_ms: self.config.read_timeout_ms,
            }
            .into());
        }

        debug!("Received {} bytes from ECU", buf.len());
        Ok(buf)
    }

    /// Discard bytes from `conn` until `idle_ms` of continuous silence.
    async fn drain_until_quiet(conn: &mut EcuConnection, idle_ms: u64) {
        let mut drain = [0u8; 64];
        loop {
            match timeout(Duration::from_millis(idle_ms), conn.read(&mut drain)).await {
                Ok(Ok(0)) | Err(_) => break,
                Ok(Ok(_)) => {}
                Ok(Err(_)) => break,
            }
        }
    }

    /// Attempt to reconnect with exponential backoff
    ///
    /// Returns Ok(()) if reconnection succeeded, Err if max retries exceeded
    pub async fn reconnect(&mut self) -> Result<()> {
        if self.retry_count >= self.config.max_retry_count {
            error!(
                "Maximum reconnection attempts ({}) exceeded",
                self.config.max_retry_count
            );
            return Err(SerialError::MaxRetriesExceeded(self.config.max_retry_count).into());
        }

        self.retry_count += 1;

        warn!(
            "Attempting reconnection {}/{} after {}ms delay",
            self.retry_count, self.config.max_retry_count, self.current_delay_ms
        );

        // Disconnect if still connected
        self.disconnect().await;

        // Wait with exponential backoff
        sleep(Duration::from_millis(self.current_delay_ms)).await;

        // Try to reconnect
        match self.connect().await {
            Ok(_) => {
                info!("Reconnection successful");
                self.retry_count = 0;
                self.current_delay_ms = self.config.initial_retry_delay_ms;
                Ok(())
            }
            Err(e) => {
                // Increase delay for next attempt (exponential backoff)
                self.current_delay_ms =
                    (self.current_delay_ms * 2).min(self.config.max_retry_delay_ms);

                warn!("Reconnection attempt failed: {}", e);
                Err(e)
            }
        }
    }

    /// Check whether the connection target exists / is reachable.
    ///
    /// For TCP mode, always returns `true` (TCP connectivity is determined at
    /// connect time).  For serial, walks the available-ports list.
    pub fn check_device_exists(&self) -> bool {
        if self.config.connection_type == "tcp" {
            return true;
        }
        match tokio_serial::available_ports() {
            Ok(ports) => {
                let exists = ports.iter().any(|p| p.port_name == self.config.port_name);
                if !exists {
                    debug!(
                        "Device {} not found in available ports",
                        self.config.port_name
                    );
                }
                exists
            }
            Err(e) => {
                warn!("Failed to enumerate serial ports: {}", e);
                false
            }
        }
    }

    /// Reset retry counter (call after successful operations)
    pub fn reset_retry_count(&mut self) {
        if self.retry_count > 0 {
            debug!("Resetting retry counter");
            self.retry_count = 0;
            self.current_delay_ms = self.config.initial_retry_delay_ms;
        }
    }

    /// Get current retry attempt number
    pub fn get_retry_count(&self) -> u32 {
        self.retry_count
    }
}

impl Drop for EcuSerialHandler {
    fn drop(&mut self) {
        debug!("Dropping EcuSerialHandler (connection will close automatically)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ecu_serial_handler_creation() {
        let config = AppConfig::default();
        let handler = EcuSerialHandler::new(config);

        assert!(!handler.is_connected());
        assert_eq!(handler.get_retry_count(), 0);
    }

    #[test]
    fn test_retry_count_management() {
        let config = AppConfig::default();
        let mut handler = EcuSerialHandler::new(config);

        handler.retry_count = 5;
        assert_eq!(handler.get_retry_count(), 5);

        handler.reset_retry_count();
        assert_eq!(handler.get_retry_count(), 0);
    }

    #[test]
    fn test_disconnected_state() {
        let config = AppConfig::default();
        let mut handler = EcuSerialHandler::new(config);

        // Should fail when not connected
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let result = handler.read_engine_data().await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_exponential_backoff_calculation() {
        let mut config = AppConfig::default();
        config.initial_retry_delay_ms = 1000;
        config.max_retry_delay_ms = 60000;

        let mut handler = EcuSerialHandler::new(config);

        // Simulate backoff growth
        assert_eq!(handler.current_delay_ms, 1000);

        handler.current_delay_ms =
            (handler.current_delay_ms * 2).min(handler.config.max_retry_delay_ms);
        assert_eq!(handler.current_delay_ms, 2000);

        handler.current_delay_ms =
            (handler.current_delay_ms * 2).min(handler.config.max_retry_delay_ms);
        assert_eq!(handler.current_delay_ms, 4000);

        // Should cap at max
        for _ in 0..10 {
            handler.current_delay_ms =
                (handler.current_delay_ms * 2).min(handler.config.max_retry_delay_ms);
        }
        assert_eq!(handler.current_delay_ms, 60000);
    }

    #[tokio::test]
    async fn test_device_check() {
        let config = AppConfig::default();
        let handler = EcuSerialHandler::new(config);

        // This will check real system ports
        // Just ensure it doesn't panic
        let _ = handler.check_device_exists();
    }
}
