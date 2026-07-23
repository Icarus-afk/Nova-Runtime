# 03. CLI Reference

The Nova Runtime CLI (`novactl` or `nova`) provides a command-line interface for managing and interacting with a running Nova Runtime instance. This document details all available commands, their options, and usage.

## 1. Global Options

The following options are available for all commands:

| Option | Description | Default |
| :----- | :---------- | :------ |
| `-c, --config <CONFIG>` | Path to the `novad.toml` configuration file. | `./novad.toml` (or system/user defaults) |
| `-o, --output <OUTPUT>` | Output format. | `table` |
| `-a, --address <ADDRESS>` | Address of the Nova Runtime server. | `http://127.0.0.1:8642` |
| `--api-key <API_KEY>` | API key for authentication. | (None) |
| `-h, --help` | Print help. | |
| `-V, --version` | Print version. | |

The `--output` option supports the following formats:

*   `table`: Human-readable tables (default).
*   `json`: JSON output.
*   `yaml`: YAML output.

## 2. Commands

### `runtime`

Manage the Nova Runtime daemon.

#### `runtime status`

Show the current status of the runtime.

```bash
novactl runtime status
```

#### `runtime start`

Start the runtime daemon. Note: This prints a help message and exits, as the daemon should be started directly via the `novad` binary.

```bash
novactl runtime start
```

#### `runtime stop`

Stop the runtime daemon. Note: This prints a help message and exits, as the daemon should be stopped via `kill`.

```bash
novactl runtime stop
```

#### `runtime restart`

Restart the runtime daemon. Note: This prints a help message and exits.

```bash
novactl runtime restart
```

#### `runtime reload`

Reload the runtime configuration by sending a SIGHUP signal to the daemon.

```bash
novactl runtime reload
```

### `config`

Manage the runtime configuration.

#### `config show`

Show the current runtime configuration.

```bash
novactl config show
```

#### `config get <KEY>`

Get a specific configuration value by its dot-separated key.

```bash
novactl config get logging.level
```

#### `config set <KEY> <VALUE>`

Set a configuration value at runtime. The value is automatically converted to the correct type based on the configuration schema.

```bash
novactl config set logging.level debug
novactl config set networking.listen_port 8080
```

#### `config validate <PATH>`

Validate a TOML configuration file against the schema.

```bash
novactl config validate ./my_config.toml
```

#### `config default`

Print the built-in default configuration in TOML format.

```bash
novactl config default > novad.toml
```

### `auth`

Manage authentication and authorization.

#### `auth create-user <USERNAME> [ROLE]`

Create a new user with an optional role.

```bash
novactl auth create-user alice admin
```

#### `auth delete-user <USERNAME>`

Delete a user.

```bash
novactl auth delete-user alice
```

#### `auth list-users`

List all users.

```bash
novactl auth list-users
```

#### `auth create-api-key <NAME>`

Create a new API key.

```bash
novactl auth create-api-key myapp
```

#### `auth revoke-api-key <KEY_ID>`

Revoke an API key.

```bash
novactl auth revoke-api-key abc123
```

### `queue`

Manage message queues.

#### `queue list`

List all queues.

```bash
novactl queue list
```

#### `queue create <NAME> [--durable]`

Create a new queue.

```bash
novactl queue create myqueue --durable
```

#### `queue delete <NAME>`

Delete a queue.

```bash
novactl queue delete myqueue
```

#### `queue publish <QUEUE> <MESSAGE>`

Publish a message to a queue.

```bash
novactl queue publish myqueue "Hello, world!"
```

#### `queue consume <QUEUE> [--count N]`

Consume messages from a queue.

```bash
novactl queue consume myqueue --count 5
```

#### `queue stats <NAME>`

Show statistics for a queue.

```bash
novactl queue stats myqueue
```

### `scheduler`

Manage scheduled jobs.

#### `scheduler list`

List all jobs.

```bash
novactl scheduler list
```

#### `scheduler create <NAME> <SCHEDULE> <COMMAND>`

Create a new job.

```bash
novactl scheduler create myjob "0 0 * * *" "echo 'Hello, world!'"
```

#### `scheduler delete <NAME>`

Delete a job.

```bash
novactl scheduler delete myjob
```

#### `scheduler pause <NAME>`

Pause a job.

```bash
novactl scheduler pause myjob
```

#### `scheduler resume <NAME>`

Resume a paused job.

```bash
novactl scheduler resume myjob
```

### `search`

Manage search indexes and queries.

#### `search query <QUERY> [--collection C] [--limit N]`

Run a search query.

```bash
novactl search query "hello world" --collection mycollection --limit 10
```

#### `search create-index <NAME> <COLLECTION> <FIELD>...`

Create a new search index.

```bash
novactl search create-index myindex mycollection field1 field2
```

#### `search drop-index <NAME>`

Drop a search index.

```bash
novactl search drop-index myindex
```

#### `search list-indexes`

List all search indexes.

```bash
novactl search list-indexes
```

### `blob`

Manage binary large objects (blobs).

#### `blob list [--prefix P]`

List blobs.

```bash
novactl blob list --prefix images/
```

#### `blob put <KEY> <FILE>`

Upload a file as a blob.

```bash
novactl blob put myimage.png ./image.png
```

#### `blob get <KEY> [OUTPUT_FILE]`

Download a blob.

```bash
novactl blob get myimage.png ./downloaded.png
```

#### `blob delete <KEY>`

Delete a blob.

```bash
novactl blob delete myimage.png
```

### `sql`

Execute SQL queries.

#### `sql query <SQL> [--format FMT]`

Run a SQL query.

```bash
novactl sql query "SELECT * FROM users"
```

#### `sql execute <FILE>`

Execute a SQL file.

```bash
novactl sql execute ./script.sql
```

#### `sql schema [TABLE]`

Show the schema of a table.

```bash
novactl sql schema users
```

### `db`

Manage databases and collections.

#### `db list`

List all databases.

```bash
novactl db list
```

#### `db create <NAME>`

Create a new database.

```bash
novactl db create mydb
```

#### `db drop <NAME>`

Drop a database.

```bash
novactl db drop mydb
```

#### `db collections <DATABASE>`

List collections in a database.

```bash
novactl db collections mydb
```

#### `db create-collection <DB> <COL>`

Create a new collection.

```bash
novactl db create-collection mydb mycollection
```

#### `db drop-collection <DB> <COL>`

Drop a collection.

```bash
novactl db drop-collection mydb mycollection
```

#### `db stats [DATABASE]`

Show database statistics.

```bash
novactl db stats mydb
```

### `cache`

Manage the cache.

#### `cache stats`

Show cache statistics.

```bash
novactl cache stats
```

#### `cache clear`

Clear the cache.

```bash
novactl cache clear
```

#### `cache flush`

Flush the cache to disk.

```bash
novactl cache flush
```

#### `cache list [--pattern P]`

List cache keys.

```bash
novactl cache list --pattern "user:*"
```

### `completion`

Generate shell completions.

#### `completion bash`

Generate completions for Bash.

```bash
novactl completion bash > /etc/bash_completion.d/novactl
```

#### `completion zsh`

Generate completions for Zsh.

```bash
novactl completion zsh > ~/.zsh/completions/_novactl
```

#### `completion fish`

Generate completions for Fish.

```bash
novactl completion fish > ~/.config/fish/completions/novactl.fish
```

#### `completion power-shell`

Generate completions for PowerShell.

```bash
novactl completion power-shell > novactl.ps1
```

### `run`

Run the Nova Runtime daemon directly from the CLI.

```bash
novactl run [OPTIONS]
```

#### Options

| Option | Description | Default |
| :----- | :---------- | :------ |
| `-c, --config <CONFIG>` | Path to the `novad.toml` configuration file. | `./novad.toml` (or system/user defaults) |
| `-d, --data-dir <DATA_DIR>` | Override the data directory. | (From config) |
| `-o, --output <OUTPUT>` | Output format. | `table` |
| `-a, --address <ADDRESS>` | Address of the Nova Runtime server. | `http://127.0.0.1:8642` |
| `--api-key <API_KEY>` | API key for authentication. | (None) |

## 3. Examples

### Get the current runtime status

```bash
novactl runtime status
```

### Set the log level to debug

```bash
novactl config set logging.level debug
```

### Create a new user

```bash
novactl auth create-user alice admin
```

### Publish a message to a queue

```bash
novactl queue publish myqueue "Hello, world!"
```

### Run a SQL query

```bash
novactl sql query "SELECT * FROM users"
```

### List all blobs

```bash
novactl blob list
```

### Generate Bash completions

```bash
novactl completion bash > /etc/bash_completion.d/novactl
```

## 4. Notes

*   The CLI communicates with the Nova Runtime daemon via its REST API. Ensure the daemon is running and accessible at the address specified by `--address`.
*   For commands that require authentication, use `--api-key` to provide an API key.
*   The `run` command is a convenience wrapper for starting the daemon directly from the CLI. For production use, it is recommended to run the `novad` binary directly.