# 00. Introduction to Nova Runtime

Nova Runtime (`novad`) is a high-performance, distributed application platform designed for modern backend services. It provides a unified and integrated environment for managing various data and processing subsystems, offering a consistent API and CLI for interaction.

## Key Features & Subsystems

Nova Runtime integrates the following core subsystems, all managed within a single daemon process (`novad`):

*   **Memory Management:** Efficiently manages in-memory resources for optimal performance.
*   **Storage Engine:** A persistent, transactional storage layer for data.
*   **Pipeline Executor:** A robust execution engine for processing operations, with built-in rate limiting, circuit breaking, and idempotency.
*   **Cache Manager:** A flexible caching layer with various eviction policies (LRU, LFU, TTL).
*   **Event Bus:** A distributed eventing system for inter-subsystem communication.
*   **Blob Storage:** Manages binary large objects (blobs) efficiently.
*   **Search Manager:** Provides full-text search capabilities over indexed data.
*   **SQL Engine:** A SQL-compatible interface for data manipulation, built on the internal storage engine.
*   **Queue Manager:** A message queuing system for asynchronous processing.
*   **Auth Manager:** Handles user authentication, API key management, and authorization (JWT-based).
*   **Scheduler Manager:** A cron-like job scheduler for recurring tasks.

## Interfaces

Nova Runtime provides several interfaces for management and interaction:

*   **REST API:** A comprehensive HTTP API for programmatic access to all subsystems.
*   **GraphQL API:** A partial GraphQL endpoint (conditionally compiled) for flexible data querying and mutations.
*   **Command-Line Interface (novactl):** A powerful CLI tool for administration, configuration, and monitoring.
*   **Web Dashboard:** A basic React-based web interface for monitoring and management (runnable separately).

## Architecture Highlights

*   **Modular Design:** Built with independent Rust crates, allowing for flexible component management.
*   **Configuration-Driven:** Behavior is extensively configured via a single `novad.toml` file, supporting runtime updates.
*   **Observability:** Integrated `tracing` for detailed logging and Prometheus-compatible metrics.
*   **Graceful Operations:** Supports graceful shutdown and SIGHUP-triggered configuration hot-reloads.
