#!/usr/bin/env bash
# Nova Runtime — curl quickstart (no SDK, no Node)
# Requires: novad running at http://127.0.0.1:8642 (make dev)
set -e
BASE="http://127.0.0.1:8642"
echo "→ health"
curl -s $BASE/health | jq . 2>/dev/null || curl -s $BASE/health

echo -e "\n→ SQL create + insert + query"
curl -s $BASE/api/v1/sql/execute -H 'Content-Type: application/json' -d '{"query":"CREATE TABLE IF NOT EXISTS demo (id INTEGER PRIMARY KEY, name TEXT)"}' | jq .
curl -s $BASE/api/v1/sql/execute -H 'Content-Type: application/json' -d '{"query":"INSERT INTO demo (id, name) VALUES (1, '\''alice'\'')"}' | jq . 2>/dev/null || true
curl -s $BASE/api/v1/sql/query -H 'Content-Type: application/json' -d '{"query":"SELECT * FROM demo"}' | jq .

echo -e "\n→ cache set/get"
curl -s $BASE/api/v1/cache/hello -X POST -H 'Content-Type: application/json' -d '{"value":{"msg":"world"}}' | jq .
curl -s $BASE/api/v1/cache/hello | jq .

echo -e "\n→ queue"
Q="demo-$(date +%s)"
curl -s $BASE/api/v1/queues -X POST -H 'Content-Type: application/json' -d "{\"name\":\"$Q\"}" | jq .
curl -s $BASE/api/v1/queues/$Q/messages -X POST -H 'Content-Type: application/json' -d '{"messages":[{"body":{"hello":"queue"}}]}' | jq .

echo -e "\n→ login (admin/admin123)"
curl -s $BASE/api/v1/auth/login -H 'Content-Type: application/json' -d '{"username":"admin","password":"admin123"}' | jq .

echo -e "\n✓ done"
