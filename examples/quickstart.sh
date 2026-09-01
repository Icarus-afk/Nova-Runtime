#!/usr/bin/env bash
# Nova Runtime — curl quickstart (no SDK, no Node)
# Requires: novad running at http://127.0.0.1:8642 (make dev)
# Admin password: NOVA_ADMIN_PASSWORD (or the random password printed in the boot log)
set -e
BASE="http://127.0.0.1:8642"
ADMIN_PASS="${NOVA_PASSWORD:-${NOVA_ADMIN_PASSWORD:-}}"
if [ -z "$ADMIN_PASS" ]; then
  echo "Set NOVA_PASSWORD (or start novad with NOVA_ADMIN_PASSWORD) first." >&2
  exit 1
fi
TOKEN=$(curl -s $BASE/api/v1/auth/login -H 'Content-Type: application/json' \
  -d "{\"username\":\"admin\",\"password\":\"$ADMIN_PASS\"}" | jq -r .access_token)
AUTH="Authorization: Bearer $TOKEN"

echo "→ health"
curl -s $BASE/health | jq . 2>/dev/null || curl -s $BASE/health

echo -e "\n→ SQL create + insert + query"
# Nova SQL: no IF NOT EXISTS, bare DROP works only if table exists — ignore errors below
curl -s $BASE/api/v1/sql/execute -H 'Content-Type: application/json' -H "$AUTH" -d '{"query":"DROP TABLE demo"}' >/dev/null 2>&1 || true
curl -s $BASE/api/v1/sql/execute -H 'Content-Type: application/json' -H "$AUTH" -d '{"query":"CREATE TABLE demo (id INTEGER PRIMARY KEY, name TEXT)"}' | jq .
curl -s $BASE/api/v1/sql/execute -H 'Content-Type: application/json' -H "$AUTH" -d '{"query":"INSERT INTO demo (id, name) VALUES (1, '\''alice'\'')"}' | jq .
curl -s $BASE/api/v1/sql/query -H 'Content-Type: application/json' -H "$AUTH" -d '{"query":"SELECT * FROM demo"}' | jq .

echo -e "\n→ cache set/get"
curl -s $BASE/api/v1/cache/hello -X POST -H 'Content-Type: application/json' -H "$AUTH" -d '{"value":{"msg":"world"},"ttl_ms":60000}' | jq .
curl -s $BASE/api/v1/cache/hello -H "$AUTH" | jq .

echo -e "\n→ queue"
Q="demo-$(date +%s)"
curl -s $BASE/api/v1/queues -X POST -H 'Content-Type: application/json' -H "$AUTH" -d "{\"name\":\"$Q\"}" | jq .
curl -s $BASE/api/v1/queues/$Q/messages -X POST -H 'Content-Type: application/json' -H "$AUTH" -d '{"messages":[{"body":{"hello":"queue"}}]}' | jq .

echo -e "\n→ login already done ($TOKEN)"
echo -e "\n✓ done"
