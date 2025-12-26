/// Message protocol for relay server communication
///
/// This module defines only the message types needed for the relay server.
/// The full protocol is in the main remotely application.

use serde::{Deserialize, Serialize};
use anyhow::Result;
use bytes::{Buf, BufMut, Bytes, BytesMut};

/// Maximum message size (10MB)
pub const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024;

/// Message types for relay communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Relay connection messages
    RelayRequest {
        /// Unique identifier to pair client and host connections
        uuid: String,
        /// Peer ID to connect to
        peer_id: String,
        /// Role: "client" or "host"
        role: String,
    },
    RelayResponse {
        /// Whether relay pairing succeeded
        success: bool,
        /// Optional error message
        message: Option<String>,
    },
}

impl Message {
    /// Serialize message to bytes with length prefix
    ///
    /// Format: [length: u32][payload: JSON]
    pub fn to_bytes(&self) -> Result<Bytes> {
        // Serialize to JSON
        let json = serde_json::to_vec(self)?;

        // Create buffer with length prefix
        let mut buf = BytesMut::with_capacity(4 + json.len());
        buf.put_u32(json.len() as u32);
        buf.put_slice(&json);

        Ok(buf.freeze())
    }

    /// Deserialize message from bytes
    pub fn from_bytes(mut data: Bytes) -> Result<Self> {
        // Check minimum length
        if data.len() < 4 {
            anyhow::bail!("Message too short");
        }

        // Read length
        let len = data.get_u32() as usize;

        // Validate length
        if len > MAX_MESSAGE_SIZE {
            anyhow::bail!("Message too large: {} bytes", len);
        }

        if data.remaining() < len {
            anyhow::bail!("Incomplete message: expected {}, got {}", len, data.remaining());
        }

        // Deserialize JSON
        let msg: Message = serde_json::from_slice(&data[..len])?;
        Ok(msg)
    }
}

/// Message framing for stream-based transport
pub struct MessageFramer {
    /// Buffer for partial messages
    buffer: BytesMut,
}

impl MessageFramer {
    pub fn new() -> Self {
        Self {
            buffer: BytesMut::with_capacity(65536),
        }
    }

    /// Add data to buffer and try to extract complete messages
    pub fn feed(&mut self, data: &[u8]) -> Vec<Message> {
        self.buffer.extend_from_slice(data);

        let mut messages = Vec::new();

        loop {
            // Need at least 4 bytes for length prefix
            if self.buffer.len() < 4 {
                break;
            }

            // Peek at length without consuming
            let len = u32::from_be_bytes([
                self.buffer[0],
                self.buffer[1],
                self.buffer[2],
                self.buffer[3],
            ]) as usize;

            // Validate length
            if len > MAX_MESSAGE_SIZE {
                tracing::error!("Invalid message length: {}, clearing buffer", len);
                self.buffer.clear();
                break;
            }

            // Check if we have complete message (length prefix + data)
            if self.buffer.len() < 4 + len {
                break; // Wait for more data
            }

            // Extract message bytes (including length prefix)
            let msg_bytes = self.buffer.split_to(4 + len);

            // Try to deserialize
            match Message::from_bytes(msg_bytes.freeze()) {
                Ok(msg) => messages.push(msg),
                Err(e) => {
                    tracing::error!("Failed to deserialize message: {}", e);
                }
            }
        }

        messages
    }

    /// Clear buffer
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

impl Default for MessageFramer {
    fn default() -> Self {
        Self::new()
    }
}
