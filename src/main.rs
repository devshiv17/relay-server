/// Relay Server for Remotely Application
///
/// This server accepts connections from both clients and hosts,
/// pairs them by UUID, and proxies data bidirectionally.
///
/// Unlike RustDesk's approach where clients/hosts must have ports open,
/// both parties connect TO this relay server (outbound connections only),
/// eliminating the need for firewall configuration.

mod protocol;

use anyhow::{Context, Result};
use protocol::{Message, MessageFramer};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio::time::{timeout, Duration};
use tracing::{error, info, warn};

/// Relay server port (configurable via command line)
const DEFAULT_RELAY_PORT: u16 = 8444;

/// Maximum time to wait for relay pairing
const PAIRING_TIMEOUT: Duration = Duration::from_secs(30);

/// Connection half - represents one side of a relay connection
struct ConnectionHalf {
    stream: TcpStream,
    peer_addr: SocketAddr,
    role: String,
    peer_id: String,
}

/// Relay pairing state
type RelayPairings = Arc<Mutex<HashMap<String, mpsc::Sender<ConnectionHalf>>>>;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .init();

    // Parse command line args
    let args: Vec<String> = std::env::args().collect();
    let port = if args.len() > 1 {
        args[1].parse().unwrap_or(DEFAULT_RELAY_PORT)
    } else {
        DEFAULT_RELAY_PORT
    };

    info!("🚀 Starting Remotely Relay Server on port {}", port);

    // Bind to all interfaces
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .context("Failed to bind relay server")?;

    info!("✓ Relay server listening on 0.0.0.0:{}", port);
    info!("📡 Ready to accept relay connections...");

    // Shared state: UUID -> channel to send paired connection
    let pairings: RelayPairings = Arc::new(Mutex::new(HashMap::new()));

    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                info!("New connection from {}", addr);
                let pairings = Arc::clone(&pairings);
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, addr, pairings).await {
                        error!("Connection handler error from {}: {}", addr, e);
                    }
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
            }
        }
    }
}

/// Handle incoming connection
async fn handle_connection(
    mut stream: TcpStream,
    addr: SocketAddr,
    pairings: RelayPairings,
) -> Result<()> {
    // Read relay request message (with timeout)
    let request = match timeout(Duration::from_secs(10), read_relay_request(&mut stream)).await {
        Ok(Ok(req)) => req,
        Ok(Err(e)) => {
            error!("Failed to read relay request from {}: {}", addr, e);
            send_relay_response(&mut stream, false, Some(format!("Invalid request: {}", e))).await?;
            return Err(e);
        }
        Err(_) => {
            error!("Timeout waiting for relay request from {}", addr);
            send_relay_response(&mut stream, false, Some("Request timeout".to_string())).await?;
            anyhow::bail!("Request timeout");
        }
    };

    info!(
        "Relay request from {}: UUID={}, peer_id={}, role={}",
        addr, request.uuid, request.peer_id, request.role
    );

    // Try to pair this connection
    let uuid = request.uuid.clone();
    let mut pairings_guard = pairings.lock().await;

    if let Some(tx) = pairings_guard.remove(&uuid) {
        // Found waiting peer - send this connection to complete the pair
        info!("✓ Pairing connection for UUID {}: {} ({})", uuid, addr, request.role);

        drop(pairings_guard); // Release lock before async operations

        // Send success response
        send_relay_response(&mut stream, true, None).await?;

        // Send this connection half to the waiting peer
        let half = ConnectionHalf {
            stream,
            peer_addr: addr,
            role: request.role.clone(),
            peer_id: request.peer_id.clone(),
        };

        if tx.send(half).await.is_err() {
            error!("Failed to send connection half for UUID {}", uuid);
            anyhow::bail!("Pairing channel closed");
        }

        info!("🔗 Connection paired successfully for UUID {}", uuid);
    } else {
        // First connection for this UUID - wait for peer
        info!("⏳ Waiting for peer to complete pairing for UUID {}", uuid);

        // Create channel for receiving the paired connection
        let (tx, mut rx) = mpsc::channel(1);
        pairings_guard.insert(uuid.clone(), tx);
        drop(pairings_guard); // Release lock

        // Send success response
        send_relay_response(&mut stream, true, None).await?;

        // Wait for peer connection (with timeout)
        let peer_half = match timeout(PAIRING_TIMEOUT, rx.recv()).await {
            Ok(Some(half)) => half,
            Ok(None) => {
                error!("Pairing channel closed for UUID {}", uuid);
                pairings.lock().await.remove(&uuid);
                anyhow::bail!("Pairing failed");
            }
            Err(_) => {
                warn!("Pairing timeout for UUID {} after {}s", uuid, PAIRING_TIMEOUT.as_secs());
                pairings.lock().await.remove(&uuid);
                anyhow::bail!("Pairing timeout");
            }
        };

        info!(
            "🔗 Paired UUID {}: {} ({}) <-> {} ({})",
            uuid, addr, request.role, peer_half.peer_addr, peer_half.role
        );

        // Start bidirectional proxy
        let conn1 = ConnectionHalf {
            stream,
            peer_addr: addr,
            role: request.role.clone(),
            peer_id: request.peer_id.clone(),
        };

        proxy_connections(conn1, peer_half).await?;
    }

    Ok(())
}

/// Read relay request message from stream
async fn read_relay_request(stream: &mut TcpStream) -> Result<RelayRequestData> {
    // Read length prefix (4 bytes)
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;

    // Validate length
    if len > 1024 * 1024 {
        anyhow::bail!("Message too large: {} bytes", len);
    }

    // Read message data
    let mut data = vec![0u8; len];
    stream.read_exact(&mut data).await?;

    // Deserialize message
    let msg: Message = serde_json::from_slice(&data)?;

    // Extract relay request
    match msg {
        Message::RelayRequest { uuid, peer_id, role } => {
            Ok(RelayRequestData { uuid, peer_id, role })
        }
        _ => anyhow::bail!("Expected RelayRequest, got {:?}", msg),
    }
}

/// Send relay response message
async fn send_relay_response(
    stream: &mut TcpStream,
    success: bool,
    message: Option<String>,
) -> Result<()> {
    let msg = Message::RelayResponse { success, message };
    let bytes = msg.to_bytes()?;
    stream.write_all(&bytes).await?;
    stream.flush().await?;
    Ok(())
}

/// Relay request data
struct RelayRequestData {
    uuid: String,
    peer_id: String,
    role: String,
}

/// Proxy data bidirectionally between two connections
async fn proxy_connections(mut conn1: ConnectionHalf, mut conn2: ConnectionHalf) -> Result<()> {
    info!(
        "🔄 Starting proxy: {} ({}) <-> {} ({})",
        conn1.peer_addr, conn1.role, conn2.peer_addr, conn2.role
    );

    let (mut r1, mut w1) = conn1.stream.split();
    let (mut r2, mut w2) = conn2.stream.split();

    let mut buf1 = vec![0u8; 64 * 1024]; // 64KB buffer
    let mut buf2 = vec![0u8; 64 * 1024];

    let mut total_bytes_1_to_2 = 0u64;
    let mut total_bytes_2_to_1 = 0u64;

    loop {
        tokio::select! {
            // Read from conn1, write to conn2
            result = r1.read(&mut buf1) => {
                match result {
                    Ok(0) => {
                        info!("Connection {} ({}) closed", conn1.peer_addr, conn1.role);
                        break;
                    }
                    Ok(n) => {
                        if let Err(e) = w2.write_all(&buf1[..n]).await {
                            error!("Write error to {} ({}): {}", conn2.peer_addr, conn2.role, e);
                            break;
                        }
                        total_bytes_1_to_2 += n as u64;
                    }
                    Err(e) => {
                        error!("Read error from {} ({}): {}", conn1.peer_addr, conn1.role, e);
                        break;
                    }
                }
            }

            // Read from conn2, write to conn1
            result = r2.read(&mut buf2) => {
                match result {
                    Ok(0) => {
                        info!("Connection {} ({}) closed", conn2.peer_addr, conn2.role);
                        break;
                    }
                    Ok(n) => {
                        if let Err(e) = w1.write_all(&buf2[..n]).await {
                            error!("Write error to {} ({}): {}", conn1.peer_addr, conn1.role, e);
                            break;
                        }
                        total_bytes_2_to_1 += n as u64;
                    }
                    Err(e) => {
                        error!("Read error from {} ({}): {}", conn2.peer_addr, conn2.role, e);
                        break;
                    }
                }
            }
        }
    }

    info!(
        "🔌 Proxy closed: {} -> {}: {} bytes, {} -> {}: {} bytes",
        conn1.role, conn2.role, total_bytes_1_to_2,
        conn2.role, conn1.role, total_bytes_2_to_1
    );

    Ok(())
}
