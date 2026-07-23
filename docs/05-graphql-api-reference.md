# 05. GraphQL API Reference

Nova Runtime provides a GraphQL API for querying and managing the runtime. This API is conditionally compiled and available at `/graphql`.

## 1. GraphQL Playground

The GraphQL Playground interface is available at:

```
http://127.0.0.1:8642/graphql
```

## 2. Schema

The GraphQL schema is built from the following root types:

*   **Query:** For querying data.
*   **Mutation:** For making changes.
*   **Subscription:** For real-time updates (not implemented).

## 3. Queries

### `health`

Returns the health status of the runtime.

**Request:**
```graphql
query {
  health {
    status
    uptimeSeconds
    version
    subsystems {
      name
      status
    }
  }
}
```

**Response:**
```json
{
  "data": {
    "health": {
      "status": "HEALTHY",
      "uptimeSeconds": 12345,
      "version": "X.Y.Z",
      "subsystems": [
        {"name": "storage", "status": "HEALTHY"}
      ]
    }
  }
}
```

### `configuration`

Returns the runtime configuration.

**Request:**
```graphql
query {
  configuration {
    version
    logLevel
    maxConnections
    queryTimeoutMs
    subsystems {
      database {
        maxConnections
      }
      cache {
        maxMemoryMb
        evictionPolicy
      }
    }
  }
}
```

**Response:**
```json
{
  "data": {
    "configuration": {
      "version": "X.Y.Z",
      "logLevel": "info",
      "maxConnections": 1024,
      "queryTimeoutMs": 5000,
      "subsystems": {
        "database": {"maxConnections": 10},
        "cache": {"maxMemoryMb": 1024, "evictionPolicy": "Lru"}
      }
    }
  }
}
```

### `metrics`

Returns runtime metrics.

**Request:**
```graphql
query {
  metrics {
    collectedAt
    system {
      cpuUsagePercent
      memoryUsageBytes
    }
    subsystems {
      database {
        queriesTotal
        avgLatencyMs
      }
    }
  }
}
```

**Response:**
```json
{
  "data": {
    "metrics": {
      "collectedAt": "2023-01-01T00:00:00Z",
      "system": {"cpuUsagePercent": 0.0, "memoryUsageBytes": 0},
      "subsystems": {
        "database": {"queriesTotal": 42, "avgLatencyMs": 0.0}
      }
    }
  }
}
```

### `version`

Returns the runtime version.

**Request:**
```graphql
query {
  version {
    version
    buildCommit
    buildDate
    rustVersion
  }
}
```

**Response:**
```json
{
  "data": {
    "version": {
      "version": "X.Y.Z",
      "buildCommit": "abc123",
      "buildDate": "2023-01-01T00:00:00Z",
      "rustVersion": "1.85"
    }
  }
}
```

### `sqlQuery`

Execute a SQL query.

**Request:**
```graphql
query {
  sqlQuery(query: "SELECT * FROM users") {
    columns {
      name
      dataType
    }
    rows
    rowCount
    executionTimeMs
  }
}
```

**Response:**
```json
{
  "data": {
    "sqlQuery": {
      "columns": [
        {"name": "id", "dataType": "TEXT"},
        {"name": "name", "dataType": "TEXT"}
      ],
      "rows": [
        {"id": 1, "name": "Alice"},
        {"id": 2, "name": "Bob"}
      ],
      "rowCount": 2,
      "executionTimeMs": 12.34
    }
  }
}
```

## 4. Mutations

### `updateConfiguration`

Update the runtime configuration.

**Request:**
```graphql
mutation {
  updateConfiguration(input: {
    logLevel: "debug",
    queryTimeoutMs: 10000
  }) {
    version
    logLevel
    queryTimeoutMs
  }
}
```

**Response:**
```json
{
  "data": {
    "updateConfiguration": {
      "version": "X.Y.Z",
      "logLevel": "debug",
      "queryTimeoutMs": 10000
    }
  }
}
```

### `setLogLevel`

Set the log level.

**Request:**
```graphql
mutation {
  setLogLevel(level: "debug") {
    version
    logLevel
  }
}
```

**Response:**
```json
{
  "data": {
    "setLogLevel": {
      "version": "X.Y.Z",
      "logLevel": "debug"
    }
  }
}
```

### `sqlExecute`

Execute a SQL statement.

**Request:**
```graphql
mutation {
  sqlExecute(query: "INSERT INTO users (name) VALUES ('Alice')") {
    rowCount
    executionTimeMs
  }
}
```

**Response:**
```json
{
  "data": {
    "sqlExecute": {
      "rowCount": 1,
      "executionTimeMs": 8.23
    }
  }
}
```

## 5. Types

### `HealthStatus`

```graphql
type HealthStatus {
  status: HealthState!
  uptimeSeconds: Int!
  version: String!
  subsystems: [SubsystemHealth!]!
  lastStartup: String!
}
```

### `ServerConfiguration`

```graphql
type ServerConfiguration {
  version: String!
  buildMode: String!
  logLevel: String!
  maxConnections: Int!
  queryTimeoutMs: Int!
  subsystems: SubsystemConfigs!
}
```

### `MetricsSnapshot`

```graphql
type MetricsSnapshot {
  collectedAt: String!
  timeRange: MetricsTimeRange!
  system: SystemMetrics!
  subsystems: SubsystemMetrics!
}
```

### `VersionInfo`

```graphql
type VersionInfo {
  version: String!
  buildCommit: String!
  buildDate: String!
  rustVersion: String!
}
```

### `SqlQueryResult`

```graphql
type SqlQueryResult {
  columns: [ColumnInfo!]!
  rows: [JSON!]!
  rowCount: Int!
  executionTimeMs: Float!
}
```

## 6. Input Types

### `ConfigurationInput`

```graphql
input ConfigurationInput {
  logLevel: String
  queryTimeoutMs: Int
  maxConnections: Int
}
```

## 7. Enums

### `HealthState`

```graphql
enum HealthState {
  HEALTHY
  DEGRADED
  UNAVAILABLE
}
```

## 8. Notes

*   The GraphQL API is conditionally compiled and may not be available in all builds.
*   The GraphQL schema is currently limited to runtime and SQL operations. Other subsystems (cache, queue, scheduler, search, blob) are not fully exposed via GraphQL.
*   For full functionality, use the REST API or CLI.