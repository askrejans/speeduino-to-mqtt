//! ECU connection abstraction.
//!
//! Provides [`EcuConnection`], an enum that wraps either a hardware serial port
//! ([`tokio_serial::SerialStream`]) or a raw TCP socket ([`tokio::net::TcpStream`]).
//! Both variants implement [`tokio::io::AsyncRead`] and [`tokio::io::AsyncWrite`], so the
//! rest of the code can use them interchangeably.

use crate::config::AppConfig;
use crate::errors::{Result, SerialError};
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
use tokio_serial::{SerialPort, SerialPortBuilderExt, SerialStream};
use tracing::info;

/// Unified ECU connection that is either hardware serial or raw TCP.
pub enum EcuConnection {
    Serial(SerialStream),
    Tcp(TcpStream),
}

impl EcuConnection {
    /// Open a connection according to the application configuration.
    pub async fn open(config: &AppConfig) -> Result<Self> {
        match config.connection_type.to_lowercase().as_str() {
            "tcp" => {
                let host = config.tcp_host.as_deref().unwrap_or("");
                let port = config.tcp_port.unwrap_or(4096);
                let addr = format!("{}:{}", host, port);
                info!("Connecting to ECU via TCP at {}", addr);
                let stream =
                    TcpStream::connect(&addr)
                        .await
                        .map_err(|e| SerialError::TcpConnectFailed {
                            addr: addr.clone(),
                            source: e,
                        })?;
                // Disable Nagle algorithm for lower latency
                stream.set_nodelay(true).ok();
                info!("TCP connection established to {}", addr);
                Ok(EcuConnection::Tcp(stream))
            }
            _ => {
                info!(
                    "Opening serial port {} at {} baud",
                    config.port_name, config.baud_rate
                );
                let port = tokio_serial::new(&config.port_name, config.baud_rate)
                    .timeout(Duration::from_millis(config.read_timeout_ms))
                    .open_native_async()
                    .map_err(|e| SerialError::OpenFailed {
                        port: config.port_name.clone(),
                        source: e,
                    })?;
                info!("Serial port opened successfully");
                Ok(EcuConnection::Serial(port))
            }
        }
    }

    /// True for TCP connections (device-existence checks are skipped for TCP).
    #[allow(dead_code)]
    pub fn is_tcp(&self) -> bool {
        matches!(self, EcuConnection::Tcp(_))
    }

    /// Attempt to clear input/output buffers.  No-op for TCP connections.
    pub fn clear_buffers(&self) -> io::Result<()> {
        if let EcuConnection::Serial(port) = self {
            port.clear(tokio_serial::ClearBuffer::All)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// AsyncRead / AsyncWrite delegation
// Both SerialStream and TcpStream are Unpin, so Pin::new() is safe.
// ---------------------------------------------------------------------------

impl AsyncRead for EcuConnection {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            EcuConnection::Serial(s) => Pin::new(s).poll_read(cx, buf),
            EcuConnection::Tcp(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for EcuConnection {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            EcuConnection::Serial(s) => Pin::new(s).poll_write(cx, buf),
            EcuConnection::Tcp(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            EcuConnection::Serial(s) => Pin::new(s).poll_flush(cx),
            EcuConnection::Tcp(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            EcuConnection::Serial(s) => Pin::new(s).poll_shutdown(cx),
            EcuConnection::Tcp(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}
