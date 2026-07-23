# 08. Testing and Simulation

This document covers testing Nova Runtime, including unit tests, integration tests, and the built-in simulator (`nova-sim`).

## 1. Running Tests

### Unit and Integration Tests

Run all tests with:

```bash
cargo test
```

Run tests for a specific crate:

```bash
cargo test -p nova-api
```

Run tests with coverage (requires `grcov` and `llvm-tools-preview`):

```bash
RUSTFLAGS="-Cinstrument-coverage" LLVM_PROFILE_FILE="nova-%p-%m.profraw" cargo test
grcov . -s . --binary-path ./target/debug/ -t html --branch --ignore-not-existing -o ./target/debug/coverage
```

### Test Structure

Tests are organized by crate:

*   `nova-api`: API route and handler tests.
*   `nova-auth`: Authentication and authorization tests.
*   `nova-blob`: Blob storage tests.
*   `nova-cache`: Cache tests.
*   `nova-cli`: CLI command tests.
*   `nova-config`: Configuration validation tests.
*   `nova-core`: Core data structure tests.
*   `nova-event`: Event bus tests.
*   `nova-executor`: Pipeline executor tests.
*   `nova-gql`: GraphQL schema and resolver tests.
*   `nova-memory`: Memory manager tests.
*   `nova-object`: Object model tests.
*   `nova-queue`: Queue manager tests.
*   `nova-scheduler`: Scheduler tests.
*   `nova-search`: Search engine tests.
*   `nova-security`: Security policy tests.
*   `nova-sql`: SQL engine tests.
*   `nova-storage`: Storage engine tests.
*   `novad`: Daemon integration tests.
*   `nova-sim`: Simulator tests.

## 2. Nova Simulator (`nova-sim`)

The Nova Simulator is a tool for testing and benchmarking Nova Runtime. It can run in two modes:

*   **TUI Mode:** Interactive terminal UI.
*   **Headless Mode:** Non-interactive, scriptable mode for automation.

### Building the Simulator

```bash
cargo build -p nova-sim --release
```

### Running in TUI Mode

```bash
cargo run -p nova-sim -- --tui
```

### Running in Headless Mode

The headless mode is useful for automated testing and benchmarking. It runs for a specified number of ticks and outputs a JSON report.

```bash
cargo run -p nova-sim -- --headless --ticks 1000 --output sim-results.json
```

#### Headless Mode Options

| Option | Description | Default |
| :----- | :---------- | :------ |
| `--headless` | Run in headless mode (no TUI). | `
| `--ticks N` | Number of simulation ticks to run. | `1000` |
| `--output FILE` | Path to write the JSON results. | `sim-results.json` |
| `--verbose` | Enable verbose logging. | (Disabled) |

#### Headless Mode Output

The JSON output includes:

*   **Summary:** Total requests, success/failure counts, latency statistics.
*   **Logs:** Detailed log of all operations, including timestamps, endpoints, and results.

Example output structure:

```json
{
  "summary": {
    "total_requests": 369,
    "success_count": 369,
    "failure_count": 0,
    "avg_latency_ms": 0.3,
    "p50_latency_ms": 0.2,
    "p95_latency_ms": 0.5,
    "p99_latency_ms": 1.1,
    "endpoints": {
      "GET /api/v1/queues": {
        "count": 12,
        "avg_latency_ms": 0.2
      },
      "POST /api/v1/auth/login": {
        "count": 1,
        "avg_latency_ms": 611.0
      }
    }
  },
  "logs": [
    {
      "timestamp": "2023-01-01T00:00:00Z",
      "tick": 1,
      "endpoint": "GET /health",
      "method": "GET",
      "status": 200,
      "latency_ms": 0.2,
      "success": true
    }
  ]
}
```

### Simulator Configuration

The simulator's behavior is configured in `crates/nova-sim/src/subsys.rs`. Key parameters:

*   **Endpoint Definitions:** The list of endpoints to test, their methods, and expected responses.
*   **Request Generation:** How requests are generated and sequenced.
*   **Concurrency:** Number of concurrent workers.

### Example: Testing a Specific Endpoint

To focus the simulator on a specific endpoint (e.g., `/api/v1/queues`), modify the `EndpointDef` list in `crates/nova-sim/src/subsys.rs` to include only the endpoints you want to test.

## 3. Benchmarking

Use the simulator in headless mode for benchmarking:

```bash
# Run 10,000 ticks and save results
cargo run -p nova-sim --release -- --headless --ticks 10000 --output bench-results.json

# Analyze results (e.g., with jq)
jq '.summary' bench-results.json
```

## 4. CI/CD

The project includes a GitHub Actions workflow (`.github/workflows/ci.yml`) for continuous integration. The workflow:

1.  Runs `cargo check` and `cargo test` on push/pull request.
2.  Builds the Docker image.
3.  (Optional) Deploys to a staging environment.

## 5. Notes

*   The simulator uses `reqwest::blocking` with a 2-second timeout for HTTP requests.
*   All tests and the simulator assume the Nova Runtime daemon is running at `127.0.0.1:8642`.
*   For end-to-end testing, start the daemon before running the simulator:
    ```bash
    cargo run -p novad --release &
    cargo run -p nova-sim -- --headless --ticks 1000
    ```