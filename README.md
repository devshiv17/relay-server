# Remotely Relay Server

A standalone relay server for the Remotely remote desktop application. This server enables peer-to-peer connections when direct UDP hole punching fails due to strict firewalls or NAT configurations.

## How It Works

Unlike traditional relay servers that require one peer to have an open port, this relay server uses a **connection pairing** approach:

1. Both client and host connect TO the relay server (outbound connections only)
2. Connections are paired using a UUID shared by both parties
3. The server proxies data bidirectionally between paired connections
4. No port forwarding or firewall configuration needed on either end

## Building

```bash
cargo build --release
```

## Running Locally

```bash
# Run on default port 8444
cargo run --release

# Run on custom port
cargo run --release -- 9000
```

## Deployment to DigitalOcean App Platform

### Prerequisites
- GitHub account
- DigitalOcean account

### Steps

1. **Push to GitHub** (already done)

2. **Create DigitalOcean App**:
   - Go to https://cloud.digitalocean.com/apps
   - Click "Create App"
   - Select "GitHub" as source
   - Authorize DigitalOcean to access your repository
   - Select the `relay-server` repository
   - Branch: `main` (or your default branch)

3. **Configure Build Settings**:
   - **Type**: Web Service
   - **Build Command**: `cargo build --release`
   - **Run Command**: `./target/release/relay-server 8080`
   - **HTTP Port**: `8080` (App Platform requires HTTP port)

4. **Configure Resources**:
   - **Size**: Basic ($5/month) is sufficient for testing
   - **Instances**: Start with 1, scale as needed

5. **Environment Variables** (optional):
   - `RUST_LOG=info` for logging level

6. **Deploy**:
   - Click "Next" and review settings
   - Click "Create Resources"
   - Wait for build and deployment (5-10 minutes)

### After Deployment

Your relay server will be available at:
```
https://your-app-name.ondigitalocean.app:8080
```

**Important**: Note the hostname for configuring your Remotely client and host applications.

### Configure Remotely App

Set the relay server address in your Remotely applications:

```bash
# On client machine
export RELAY_SERVER=your-app-name.ondigitalocean.app:8080

# On host machine
export RELAY_SERVER=your-app-name.ondigitalocean.app:8080
```

Or configure it in the application settings.

## Monitoring

View logs in DigitalOcean App Platform:
- Go to your app in the dashboard
- Click "Runtime Logs" tab
- Monitor connection attempts, pairings, and proxy traffic

## Architecture

```
┌─────────┐                    ┌──────────────┐                    ┌─────────┐
│ Client  │───outbound tcp────>│ Relay Server │<───outbound tcp────│  Host   │
└─────────┘                    └──────────────┘                    └─────────┘
                                      │
                                      ├─ Pair by UUID
                                      ├─ Proxy bidirectionally
                                      └─ No inbound ports needed
```

## Protocol

Messages use length-prefixed JSON:
- `[4 bytes: length][JSON payload]`
- Initial handshake: `RelayRequest` with UUID, peer_id, role
- Response: `RelayResponse` with success status
- After pairing: raw TCP proxy (transparent to applications)

## Performance

- 64KB buffers for efficient proxying
- Async I/O with Tokio runtime
- Minimal overhead (just TCP forwarding after pairing)
- Suitable for video streaming (tested with H.264)

## Troubleshooting

### Connection timeout
- Check firewall allows outbound TCP to port 8080
- Verify relay server is running (check logs)
- Ensure both peers use same UUID

### Pairing fails
- Default pairing timeout is 30 seconds
- Both peers must connect within this window
- Check that peer IDs and UUIDs match

### High latency
- Relay adds one network hop vs direct P2P
- Consider deploying closer to your users geographically
- Monitor relay server resource usage

## License

MIT
