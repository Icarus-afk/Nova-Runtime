# CLI — `novactl`

Binary: `target/debug/novactl` (`make setup` builds) or `target/release/novactl` (`make build`).

Global flags: `--config PATH`, `--output table|json|yaml`, `--address http://127.0.0.1:8642` (or `NOVA_URL`/`NOVA_API_URL` env), `--api-key`.

```bash
novactl --help
novactl runtime status
novactl config show
novactl config get storage.page_cache_size
novactl config set logging.level debug
novactl config default > novad.toml
novactl config validate ./novad.toml
```

Subcommands:

- `runtime` `status|start|stop|restart|reload`
- `auth` `create-user <name> [role]` `delete-user` `list-users` `create-api-key <name>` `revoke-api-key`
- `sql` `query "SELECT 1" --format json` `execute script.sql` `schema [table]`
- `db` `list|create|drop|collections|create-collection|drop-collection|stats`
- `cache` `get/set/delete/list --pattern "user:*"|stats|clear|flush`
- `queue` `list|create <name> [--durable]|delete|publish|consume --count 5|stats`
- `scheduler` `list|create <name> "<cron>" <cmd>|delete|pause|resume`
- `search` `query <q> --collection docs --limit 10|create-index|drop-index|list-indexes`
- `blob` `list --prefix img/|put <key> <file>|get <key> [out]|delete`
- `completion` `bash|zsh|fish|power-shell`

Examples:

```bash
novactl --address http://127.0.0.1:8642 --api-key nr_xxx sql query "SELECT * FROM demo WHERE id=\$1" --output json
NOVA_API_KEY=nr_xxx novactl queue publish myq '{"hello":1}'
```

See `crates/nova-cli/src/app.rs:1`.
