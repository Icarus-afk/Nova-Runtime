# 07. Deployment

This document covers deployment options for Nova Runtime, including binary deployment, Docker, and development scripts.

## 1. Binary Deployment

### Building from Source

1.  Ensure you have the Rust toolchain installed (see [rustup.rs](https://rustup.rs/) for installation instructions).
2.  Clone the Nova Runtime repository:
    ```bash
    git clone https://github.com/Icarus-afk/Nova-Runtime.git
    cd Nova-Runtime
    ```
3.  Build the release binary:
    ```bash
    cargo build --release
    ```
    The compiled binary will be available at `target/release/novad`.

### Running the Binary

1.  Copy the binary to your server:
    ```bash
    scp target/release/novad user@server:/usr/local/bin/novad
    ```
2.  Create a configuration file (see [Configuration](02-configuration.md)):
    ```bash
    mkdir -p /etc/novad
    novad config default > /etc/novad/novad.toml
    ```
3.  Run the daemon:
    ```bash
    novad --config /etc/novad/novad.toml
    ```

### Systemd Service

To run Nova Runtime as a systemd service:

1.  Create a service file at `/etc/systemd/system/novad.service`:
    ```ini
    [Unit]
    Description=Nova Runtime Daemon
    After=network.target

    [Service]
    User=novad
    Group=novad
    ExecStart=/usr/local/bin/novad --config /etc/novad/novad.toml
    Restart=always
    RestartSec=5
    LimitNOFILE=4096

    [Install]
    WantedBy=multi-user.target
    ```
2.  Create a user for the service:
    ```bash
    useradd -r -s /bin/false novad
    ```
3.  Enable and start the service:
    ```bash
    systemctl daemon-reload
    systemctl enable novad
    systemctl start novad
    ```

## 2. Docker Deployment

### Building the Docker Image

1.  Build the image from the project root:
    ```bash
    docker build -t nova-runtime .
    ```

### Running the Container

1.  Run the container with a bind mount for persistent data:
    ```bash
    docker run -d \
      --name nova-runtime \
      -p 8642:8642 \
      -v /path/to/data:/var/lib/novad \
      -v /path/to/config:/etc/novad \
      nova-runtime
    ```

### Docker Compose

1.  Use the provided `docker-compose.yml`:
    ```bash
    docker-compose up -d
    ```

## 3. Development Scripts

The project includes several scripts in the `scripts/` directory for development and testing:

| Script | Description |
| :----- | :---------- |
| `setup.sh` | Install dependencies and set up the development environment. |
| `dev.sh` | Start the Nova Runtime daemon and dashboard in development mode. |
| `seed.sh` | Populate the database with test data. |
| `test.sh` | Run all tests. |

### Running the Development Environment

1.  Make the scripts executable:
    ```bash
    chmod +x scripts/*.sh
    ```
2.  Set up the environment:
    ```bash
    ./scripts/setup.sh
    ```
3.  Start the development server:
    ```bash
    ./scripts/dev.sh
    ```

## 4. Configuration

Refer to the [Configuration](02-configuration.md) document for details on configuring Nova Runtime via `novad.toml`.

## 5. Upgrading

### Binary Upgrade

1.  Stop the running daemon:
    ```bash
    systemctl stop novad
    ```
2.  Replace the binary with the new version:
    ```bash
    scp target/release/novad user@server:/usr/local/bin/novad
    ```
3.  Restart the daemon:
    ```bash
    systemctl start novad
    ```

### Docker Upgrade

1.  Stop and remove the old container:
    ```bash
    docker stop nova-runtime
    docker rm nova-runtime
    ```
2.  Pull the new image (if using a remote registry) or rebuild:
    ```bash
    docker pull nova-runtime:latest
    # or
    docker build -t nova-runtime .
    ```
3.  Start the new container:
    ```bash
    docker run -d \
      --name nova-runtime \
      -p 8642:8642 \
      -v /path/to/data:/var/lib/novad \
      -v /path/to/config:/etc/novad \
      nova-runtime
    ```

## 6. Monitoring

### Logs

*   **Binary:** Logs are written to stdout/stderr by default. Use `journalctl` for systemd:
    ```bash
    journalctl -u novad -f
    ```
*   **Docker:** Use `docker logs`:
    ```bash
    docker logs -f nova-runtime
    ```

### Metrics

Prometheus-compatible metrics are available at:

```
http://127.0.0.1:8642/metrics
```

### Health Checks

*   **Liveness:** `http://127.0.0.1:8642/live`
*   **Readiness:** `http://127.0.0.1:8642/ready`
*   **Health:** `http://127.0.0.1:8642/health`

## 7. Notes

*   Nova Runtime stores all persistent data in the directory specified by `general.data_dir` in `novad.toml`. Ensure this directory is backed up regularly.
*   The default listen port is `8642`. Change this in `novad.toml` if needed.
*   For production deployments, consider:
  *   Running behind a reverse proxy (e.g., Nginx, HAProxy).
  *   Enabling TLS (not fully implemented in the current version).
  *   Setting up log rotation for the daemon logs.