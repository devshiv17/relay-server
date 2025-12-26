# Deployment Guide - Linux Server

## Deploy Both Relay and Signal Server on Same Machine

### 1. Copy Files to Server

```bash
# Copy relay server
scp -r relay-server user@YOUR_SERVER_IP:/home/user/

# Copy signal server (from your remotely_app project)
scp -r signal_server user@YOUR_SERVER_IP:/home/user/
```

### 2. SSH into Server and Build

```bash
ssh user@YOUR_SERVER_IP

# Install Rust if not installed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Build relay server
cd ~/relay-server
cargo build --release

# Build signal server
cd ~/signal_server
cargo build --release
```

### 3. Setup Systemd Services

```bash
# Edit service files to replace YOUR_USERNAME with your actual username
cd ~/relay-server
sed -i "s/YOUR_USERNAME/$USER/g" relay-server.service
sed -i "s/YOUR_USERNAME/$USER/g" signal-server.service

# Copy service files to systemd
sudo cp relay-server.service /etc/systemd/system/
sudo cp signal-server.service /etc/systemd/system/

# Reload systemd
sudo systemctl daemon-reload

# Enable services (start on boot)
sudo systemctl enable relay-server
sudo systemctl enable signal-server

# Start services
sudo systemctl start relay-server
sudo systemctl start signal-server

# Check status
sudo systemctl status relay-server
sudo systemctl status signal-server
```

### 4. Configure Firewall

```bash
# Allow ports
sudo ufw allow 8080/tcp  # Signal server
sudo ufw allow 8444/tcp  # Relay server

# Enable firewall if not already
sudo ufw enable
```

### 5. Verify Services Running

```bash
# Check if ports are listening
sudo netstat -tlnp | grep -E '8080|8444'

# View logs
sudo journalctl -u relay-server -f
sudo journalctl -u signal-server -f
```

### 6. Configure Clients

On your client and host machines, set environment variables:

```bash
export SIGNAL_SERVER=YOUR_SERVER_IP:8080
export RELAY_SERVER=YOUR_SERVER_IP:8444
```

Or configure in your app settings.

## Service Management Commands

```bash
# Start services
sudo systemctl start relay-server
sudo systemctl start signal-server

# Stop services
sudo systemctl stop relay-server
sudo systemctl stop signal-server

# Restart services
sudo systemctl restart relay-server
sudo systemctl restart signal-server

# View status
sudo systemctl status relay-server
sudo systemctl status signal-server

# View logs (live)
sudo journalctl -u relay-server -f
sudo journalctl -u signal-server -f

# View logs (last 100 lines)
sudo journalctl -u relay-server -n 100
sudo journalctl -u signal-server -n 100

# Disable services (prevent auto-start)
sudo systemctl disable relay-server
sudo systemctl disable signal-server
```

## Port Configuration

- **Signal Server**: Port 8080 (default)
- **Relay Server**: Port 8444 (default)

To change ports, edit the service files in `/etc/systemd/system/` and restart services.

## Troubleshooting

### Service won't start
```bash
# Check service logs
sudo journalctl -u relay-server -n 50
sudo journalctl -u signal-server -n 50

# Check if binary exists
ls -l ~/relay-server/target/release/relay-server
ls -l ~/signal_server/target/release/signal_server

# Check permissions
sudo chmod +x ~/relay-server/target/release/relay-server
sudo chmod +x ~/signal_server/target/release/signal_server
```

### Port already in use
```bash
# Find what's using the port
sudo lsof -i :8080
sudo lsof -i :8444

# Kill the process if needed
sudo kill <PID>
```

### Can't connect from client
```bash
# Check firewall
sudo ufw status

# Test from server itself
curl http://localhost:8080
nc -zv YOUR_SERVER_IP 8444

# Test from client machine
nc -zv YOUR_SERVER_IP 8080
nc -zv YOUR_SERVER_IP 8444
```

## Security Recommendations

1. **Use a reverse proxy** (nginx) for HTTPS
2. **Enable firewall** - only allow necessary ports
3. **Keep system updated**: `sudo apt update && sudo apt upgrade`
4. **Monitor logs** regularly for suspicious activity
5. **Consider fail2ban** to prevent brute force attacks

## Performance Tuning

For production use, consider:
- Increasing file descriptor limits
- Tuning TCP keepalive settings
- Setting up log rotation
- Monitoring with tools like Prometheus
