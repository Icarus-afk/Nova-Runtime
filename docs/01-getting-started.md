# 01. Getting Started with Nova Runtime

This guide will walk you through building, configuring, and running the Nova Runtime daemon (`novad`).

## 1. Prerequisites

*   **Rust Toolchain:** Ensure you have Rust and Cargo installed. If not, follow the instructions at [rustup.rs](https://rustup.rs/).

## 2. Building the Daemon

Navigate to the project root directory and build `novad`:

```bash
cargo build --release
```

This command compiles the daemon in release mode, producing an optimized executable in `target/release/novad`. For development and debugging, you can use `cargo build` (producing `target/debug/novad`).

## 3. Configuration (novad.toml)

Nova Runtime is primarily configured via a TOML file named `novad.toml`. A default configuration is embedded within the daemon, but you will typically provide your own. You can find an example `novad.toml` in the project root (or generate the default with `novactl config default`).

### Configuration File Resolution Order

`novad` looks for its configuration file in the following order:

1.  **Command-line argument:** Path specified by the `--config <PATH>` flag.
2.  **Local Directory:** `./novad.toml` in the current working directory.
3.  **User Configuration Directory (XDG):** `$XDG_CONFIG_HOME/nova/novad.toml` (if `XDG_CONFIG_HOME` is set).
4.  **User Configuration Directory (Home):** `~/.config/nova/novad.toml` (if `HOME` is set).
5.  **System Configuration Directory:** `/etc/novad/novad.toml`.

### Example Minimal `novad.toml`

```toml
[general]
data_dir = "./nova_data"

[networking]
listen_address = "127.0.0.1"
listen_port = 8642

[logging]
level = "info"
format = "json" # or "compact"
```

## 4. Running the Daemon

After building, you can run `novad` from the `target/release/` (or `target/debug/`) directory:

```bash
# Run with default configuration
target/release/novad

# Run with a custom configuration file
target/release/novad --config /path/to/your/novad.toml

# Override data directory and listen address/port
target/release/novad --data-dir /var/lib/nova --listen 0.0.0.0:8080

# Set log level to debug
target/release/novad --log-level debug
```

Upon startup, you will see a console banner and log messages indicating the daemon's status:

```
  ╔══════════════════════════════════════╗
  ║         Nova Runtime vX.Y.Z          ║
  ║     Status: RUNNING                   ║
  ║     Listen: 127.0.0.1:8642            ║
  ║     GraphQL: /graphql                 ║
  ║     PID:    <PROCESS_ID>              ║
  ╚══════════════════════════════════════╝
```

### CLI Arguments

| Argument | Description | Default (if not specified in config) |
| :------- | :---------- | :----------------------------------- |
| `--config <PATH>` | Path to the `novad.toml` configuration file. | Searched in predefined locations. |
| `--data-dir <PATH>` | Overrides `general.data_dir` in config. | `./nova_data` (from default config) |
| `--listen <ADDR:PORT>` | Overrides `networking.listen_address` and `networking.listen_port` in config. | `127.0.0.1:8642` (from default config) |
| `--log-level <LEVEL>` | Sets the logging level (`info`, `debug`, `trace`, `warn`, `error`). | `info` |

## 5. Hot Reloading Configuration

You can trigger `novad` to reload its configuration from the original `novad.toml` file without restarting the daemon by sending a `SIGHUP` signal:

```bash
kill -SIGHUP <novad_PID>
```

The daemon will attempt to re-parse the config file. If successful, new settings (where applicable) will be applied. If parsing fails, the old configuration remains active, and an error is logged.

## 6. Graceful Shutdown

To gracefully shut down `novad`, send an interrupt signal (e.g., `Ctrl+C` in the terminal where it's running, or `kill <novad_PID>`):

```bash
# In the terminal running novad
Ctrl+C

# Or from another terminal
kill <novad_PID>
```

The daemon will attempt to drain its execution pipeline, close storage, and perform other cleanup tasks before exiting.
