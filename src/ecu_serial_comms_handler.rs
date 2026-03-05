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

/// Minimum bytes we must see to have a valid primary-serial packet.
const MIN_PACKET_BYTES: usize = 130;
/// After reading `expected_data_length` bytes, keep draining until this many ms
/// of silence.  Handles firmware that sends more bytes than the configured
/// length (e.g. real ECU = 138 bytes, config default = 130).
const POST_DRAIN_IDLE_MS: u64 = 25;
/// Cap on extra bytes collected by the post-drain loop.
const MAX_EXTRA_BYTES: usize = 64;

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
    /// Strategy:
    ///   1. `clear_buffers()` — instant kernel FIFO purge, discards any stale
    ///      bytes left over from a previous cycle or a previous run.
    ///   2. Send `A`, `flush()`.
    ///   3. `read_exact(expected_data_length)` — deterministic; blocks until
    ///      every byte is received.
    ///   4. Post-drain — collect any additional bytes that arrive within
    ///      `POST_DRAIN_IDLE_MS` ms (e.g. real ECU sends 138, config says 130;
    ///      the extra 8 bytes are drained here so they never pollute the next
    ///      cycle).
    pub async fn read_engine_data(&mut self) -> Result<Vec<u8>> {
        let conn = self.connection.as_mut().ok_or(SerialError::Disconnected)?;

        // ── 1. Pre-clear ────────────────────────────────────────────────────
        // Kernel-level instant UART/USB FIFO purge.  No-op for TCP.
        // Eliminates stale bytes without any timing dependency.
        conn.clear_buffers().ok();

        // ── 2. Send command ─────────────────────────────────────────────────
        debug!("Sending ECU command: 0x{:02X}", ECU_COMMAND);
        conn.write_all(&[ECU_COMMAND])
            .await
            .map_err(SerialError::WriteFailed)?;
        conn.flush().await.map_err(SerialError::WriteFailed)?;

        // ── 3. read_exact ───────────────────────────────────────────────────
        let expected = self.config.expected_data_length;
        let mut buffer = vec![0u8; expected];
        match timeout(
            Duration::from_millis(self.config.read_timeout_ms),
            conn.read_exact(&mut buffer),
        )
        .await
        {
            Ok(Ok(_)) => debug!("read_exact: got {} bytes", expected),
            Ok(Err(e)) => {
                error!("Serial read error: {}", e);
                return Err(SerialError::ReadFailed(e).into());
            }
            Err(_) => {
                error!("Read timeout after {}ms", self.config.read_timeout_ms);
                return Err(SerialError::ReadTimeout {
                    timeout_ms: self.config.read_timeout_ms,
                }
                .into());
            }
        }

        // ── 4. Post-drain ───────────────────────────────────────────────────
        // Real firmware may send more bytes than `expected_data_length`.
        // Drain extras so the OS buffer is empty for the next cycle.
        let mut tmp = [0u8; 64];
        let mut extra: Vec<u8> = Vec::new();
        loop {
            match timeout(Duration::from_millis(POST_DRAIN_IDLE_MS), conn.read(&mut tmp)).await {
                Ok(Ok(0)) | Err(_) => break,
                Ok(Ok(n)) => {
                    if extra.len() + n <= MAX_EXTRA_BYTES {
                        extra.extend_from_slice(&tmp[..n]);
                    }
                }
                Ok(Err(_)) => break,
            }
        }
        if !extra.is_empty() {
            debug!("Post-drain: {} extra bytes (total {})", extra.len(), expected + extra.len());
            buffer.extend_from_slice(&extra);
        }

        if buffer.len() < MIN_PACKET_BYTES {
            warn!("Packet too short: {} bytes (expected ≥{})", buffer.len(), MIN_PACKET_BYTES);
        }

        Ok(buffer)
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
