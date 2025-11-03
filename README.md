# Crash

A Rust-based proxy core management tool supporting Clash/Mihomo/SingBox and other proxy cores. This is a Rust port of [ShellCrash](https://github.com/juewuy/ShellCrash).

## Features

- 🚀 Cross-platform support (Linux, macOS, Windows, Android)
- 📦 Automatic download and installation of proxy cores
- 🔄 Automatic configuration and GeoIP database updates
- 🌐 Multiple Web UI support (Metacubexd, Zashboard, Yacd)
- ⏰ Scheduled task support (automatic config and database updates)
- 🔧 Flexible configuration management
- 🪞 Multiple GitHub mirror support for accelerated downloads

## Installation

### Quick Install

Install with a single command using the installation script:

```bash
bash <(curl -fsSL https://raw.githubusercontent.com/ahaoboy/crash/main/install.sh)
```

### Using Proxy for Faster Downloads

If GitHub access is slow, use a mirror:

```bash
# Using gh-proxy mirror
bash <(curl -fsSL https://raw.githubusercontent.com/ahaoboy/crash/main/install.sh) --proxy gh-proxy

curl -fsSL https://gh-proxy.com/https://github.com/ahaoboy/crash/blob/main/install.sh | sh -s -- --proxy gh-proxy
curl -fsSL https://xget.xi-xu.me/gh/ahaoboy/crash/raw/refs/heads/main/install.sh | sh -s -- --proxy xget

# Using xget mirror
bash <(curl -fsSL https://raw.githubusercontent.com/ahaoboy/crash/main/install.sh) --proxy xget

# Using jsdelivr CDN
bash <(curl -fsSL https://raw.githubusercontent.com/ahaoboy/crash/main/install.sh) --proxy jsdelivr
```

### Custom Installation Directory

```bash
export EI_DIR=~/.local/bin
bash <(curl -fsSL https://raw.githubusercontent.com/ahaoboy/crash/main/install.sh)
```

### Build from Source

```bash
# Clone the repository
git clone https://github.com/ahaoboy/crash.git
cd crash

# Build
cargo build --release

# Install
cargo install --path .
```

## Usage

### Initialize and Install

```bash
# Install proxy core and UI components
crash install

# Force reinstallation
crash install --force
```

### Configuration Management

```bash
# Set configuration file URL
crash url <config-url>

# Update configuration from URL
crash update-url

# Update configuration from saved URL
crash update
```

### Service Control

```bash
# Start proxy service
crash start

# Stop proxy service
crash stop

# Check service status
crash status
```

### GeoIP Database Management

```bash
# Update GeoIP databases
crash update-geo

# Force update
crash update-geo --force
```

### Web UI Configuration

```bash
# Set Web UI type
crash ui metacubexd  # or zashboard, yacd

# Set Web controller host
crash host :9090

# Set Web controller secret
crash secret <your-secret>
```

### Scheduled Tasks

```bash
# Install scheduled tasks (auto-update config and databases)
crash task

# Manually run scheduled task
crash run-task

# Remove scheduled tasks
crash remove-task
```

### Proxy Settings

```bash
# Set GitHub download proxy
crash proxy direct      # Direct connection
crash proxy ghproxy     # gh-proxy mirror
crash proxy xget        # xget mirror
crash proxy jsdelivr    # jsdelivr CDN
```

## Configuration File

Configuration file location:
- Linux/macOS: `~/.crash/config.json`
- Windows: `%USERPROFILE%\.crash\config.json`

Example configuration:

```json
{
  "url": "https://example.com/config.yaml",
  "proxy": "Direct",
  "web": {
    "ui": "Metacubexd",
    "host": ":9090",
    "secret": "your-secret"
  }
}
```

## Supported Platforms

- Linux (x86_64, aarch64, armv7, i686) - musl/gnu
- macOS (x86_64, aarch64/Apple Silicon)
- Windows (x86_64, i686, aarch64)
- Android (aarch64, armv7, x86_64, i686)

## Scheduled Tasks

After installing scheduled tasks, the system will automatically:

- **Every Wednesday at 3:00 AM**: Update configuration files and GeoIP databases
- **Every 10 minutes**: Check and start proxy service (if not running)

### Linux/macOS (crontab)

```cron
0 3 * * 3 ~/.crash/crash run-task
*/10 * * * * ~/.crash/crash start
```

### Windows (Task Scheduler)

- `CrashRunTask`: Runs every Wednesday at 03:00
- `CrashStart`: Runs every 10 minutes

## Logging

Log files location:
- Linux/macOS: `~/.crash/logs/`
- Windows: `%USERPROFILE%\.crash\logs\`

Logs are automatically rotated, keeping the last 5 files with a maximum size of 10MB each.

## Development

### Building

```bash
# Development build
cargo build

# Release build
cargo build --release

# Run tests
cargo test
```

### Using just Commands

The project includes a `justfile`. You can use [just](https://github.com/casey/just) to simplify development:

```bash
# List available commands
just --list

# Build project
just build

# Run tests
just test
```

## License

MIT License - see [LICENSE](LICENSE) file for details

## Acknowledgments

- [ShellCrash](https://github.com/juewuy/ShellCrash) - Original project
- [Clash](https://github.com/Dreamacro/clash) - Proxy core
- [Mihomo](https://github.com/MetaCubeX/mihomo) - Clash fork
- [SingBox](https://github.com/SagerNet/sing-box) - Universal proxy platform

## Contributing

Issues and Pull Requests are welcome!

## Links

- [GitHub Repository](https://github.com/ahaoboy/crash)
- [Issue Tracker](https://github.com/ahaoboy/crash/issues)
