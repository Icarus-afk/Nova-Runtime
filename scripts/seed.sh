#!/usr/bin/env bash
set -euo pipefail

BASE="http://127.0.0.1:8642/api/v1"

echo "=== Nova Runtime Seed Script ==="
echo ""

# Login as admin
echo "--- Login ---"
LOGIN=$(curl -sf -X POST "$BASE/auth/login" \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"admin123"}')
TOKEN=$(echo "$LOGIN" | python3 -c "import sys,json; print(json.load(sys.stdin)['access_token'])")
AUTH="Authorization: Bearer $TOKEN"
echo "Token: ${TOKEN:0:20}..."
echo ""

sql() {
  local q="$1"
  curl -sf -X POST "$BASE/sql/execute" -H "$AUTH" -H 'Content-Type: application/json' \
    --data-raw "{\"query\":$q}" > /dev/null || true
}

sql_q() {
  local q="$1"
  curl -sf -X POST "$BASE/sql/query" -H "$AUTH" -H 'Content-Type: application/json' \
    --data-raw "{\"query\":$q}" > /dev/null || true
}

# Drop existing tables for a clean seed
drop() {
  curl -sf -X POST "$BASE/sql/execute" -H "$AUTH" -H 'Content-Type: application/json' \
    --data-raw "{\"query\":\"DROP TABLE $1\"}" > /dev/null || true
}

# --- SQL TABLES ---
echo "--- SQL: Dropping old tables ---"
drop "events"
drop "logs"
drop "orders"
drop "products"
drop "users"
echo "  done"
echo ""

echo "--- SQL: Creating tables ---"
sql '"CREATE TABLE users (id Integer, name Text, email Text, role Text, active Boolean, created_at Integer)"'
echo "  users"
sql '"CREATE TABLE products (id Integer, name Text, price Float, stock Integer, category Text, description Text, created_at Integer)"'
echo "  products"
sql '"CREATE TABLE orders (id Integer, user_id Integer, product_id Integer, quantity Integer, total Float, status Text, shipping_address Text, created_at Integer)"'
echo "  orders"
sql '"CREATE TABLE logs (id Integer, level Text, message Text, source Text, created_at Integer)"'
echo "  logs"
sql '"CREATE TABLE events (id Integer, name Text, category Text, payload Text, created_at Integer)"'
echo "  events"
echo ""

# --- SQL INSERTS ---
echo "--- SQL: Inserting data ---"

# Users (20)
for i in $(seq 1 20); do
  NAME="user$i"
  EMAIL="${NAME}@example.com"
  ROLE="member"
  [ $i -eq 1 ] && ROLE="admin"
  [ $i -eq 2 ] && ROLE="moderator"
  ACTIVE="true"
  [ $i -gt 18 ] && ACTIVE="false"
  TS=$((1700000000000 + i * 86400000))
  sql "\"INSERT INTO users VALUES ($i, '${NAME}', '${EMAIL}', '${ROLE}', ${ACTIVE}, ${TS})\""
done
echo "  20 users"

# Products (30)
sql '"INSERT INTO products VALUES (1, '\''Wireless Mouse'\'', 29.99, 150, '\''Electronics'\'', '\''Ergonomic 2.4GHz mouse'\'', 1700000000000)"'
sql '"INSERT INTO products VALUES (2, '\''Mechanical Keyboard'\'', 89.99, 75, '\''Electronics'\'', '\''RGB backlit mechanical'\'', 1700086400000)"'
sql '"INSERT INTO products VALUES (3, '\''USB-C Hub'\'', 45.50, 200, '\''Electronics'\'', '\''7-port USB-C hub'\'', 1700172800000)"'
sql '"INSERT INTO products VALUES (4, '\''27 Monitor'\'', 349.99, 30, '\''Electronics'\'', '\''4K IPS 27-inch'\'', 1700259200000)"'
sql '"INSERT INTO products VALUES (5, '\''Webcam HD'\'', 79.99, 100, '\''Electronics'\'', '\''1080p webcam'\'', 1700345600000)"'
sql '"INSERT INTO products VALUES (6, '\''Standing Desk'\'', 599.99, 20, '\''Furniture'\'', '\''Electric height-adjustable'\'', 1700432000000)"'
sql '"INSERT INTO products VALUES (7, '\''Office Chair'\'', 299.99, 45, '\''Furniture'\'', '\''Ergonomic mesh chair'\'', 1700518400000)"'
sql '"INSERT INTO products VALUES (8, '\''Desk Lamp'\'', 39.99, 80, '\''Furniture'\'', '\''LED lamp wireless charger'\'', 1700604800000)"'
sql '"INSERT INTO products VALUES (9, '\''Bookshelf'\'', 149.99, 25, '\''Furniture'\'', '\''5-tier wood'\'', 1700691200000)"'
sql '"INSERT INTO products VALUES (10, '\''Notebook Pro'\'', 1299.99, 50, '\''Computers'\'', '\''15.6-inch 16GB RAM'\'', 1700777600000)"'
sql '"INSERT INTO products VALUES (11, '\''Tablet Air'\'', 499.99, 60, '\''Computers'\'', '\''10.9-inch 64GB'\'', 1700864000000)"'
sql '"INSERT INTO products VALUES (12, '\''Smart Watch'\'', 249.99, 90, '\''Wearables'\'', '\''Fitness smart watch'\'', 1700950400000)"'
sql '"INSERT INTO products VALUES (13, '\''Wireless Earbuds'\'', 149.99, 120, '\''Audio'\'', '\''Noise-cancelling'\'', 1701036800000)"'
sql '"INSERT INTO products VALUES (14, '\''Bluetooth Speaker'\'', 79.99, 65, '\''Audio'\'', '\''Portable waterproof'\'', 1701123200000)"'
sql '"INSERT INTO products VALUES (15, '\''USB Microphone'\'', 119.99, 40, '\''Audio'\'', '\''Condenser mic'\'', 1701209600000)"'
sql '"INSERT INTO products VALUES (16, '\''Graphics Tablet'\'', 199.99, 35, '\''Computers'\'', '\''Drawing tablet 8x6'\'', 1701296000000)"'
sql '"INSERT INTO products VALUES (17, '\''External SSD'\'', 109.99, 85, '\''Storage'\'', '\''1TB USB-C SSD'\'', 1701382400000)"'
sql '"INSERT INTO products VALUES (18, '\''NAS Drive'\'', 249.99, 15, '\''Storage'\'', '\''2-bay 4TB'\'', 1701468800000)"'
sql '"INSERT INTO products VALUES (19, '\''HDMI Cable'\'', 12.99, 500, '\''Accessories'\'', '\''High-speed HDMI 2.1'\'', 1701555200000)"'
sql '"INSERT INTO products VALUES (20, '\''Power Strip'\'', 24.99, 300, '\''Accessories'\'', '\''6-outlet surge protector'\'', 1701641600000)"'
sql '"INSERT INTO products VALUES (21, '\''Webcam Stand'\'', 19.99, 150, '\''Accessories'\'', '\''Adjustable mount'\'', 1701728000000)"'
sql '"INSERT INTO products VALUES (22, '\''Mouse Pad'\'', 14.99, 250, '\''Accessories'\'', '\''Large desk pad'\'', 1701814400000)"'
sql '"INSERT INTO products VALUES (23, '\''Laptop Stand'\'', 34.99, 110, '\''Accessories'\'', '\''Aluminum adjustable'\'', 1701900800000)"'
sql '"INSERT INTO products VALUES (24, '\''Cable Organizer'\'', 9.99, 400, '\''Accessories'\'', '\''Velcro ties 10-pack'\'', 1701987200000)"'
sql '"INSERT INTO products VALUES (25, '\''Monitor Arm'\'', 89.99, 55, '\''Furniture'\'', '\''Gas spring arm'\'', 1702073600000)"'
sql '"INSERT INTO products VALUES (26, '\''LED Strip'\'', 29.99, 95, '\''Accessories'\'', '\''RGB USB strip 2m'\'', 1702160000000)"'
sql '"INSERT INTO products VALUES (27, '\''Air Purifier'\'', 199.99, 22, '\''Home'\'', '\''HEPA filter'\'', 1702246400000)"'
sql '"INSERT INTO products VALUES (28, '\''Plant Pot Set'\'', 34.99, 70, '\''Home'\'', '\''Ceramic 3-pack'\'', 1702332800000)"'
sql '"INSERT INTO products VALUES (29, '\''Wall Art Canvas'\'', 49.99, 40, '\''Home'\'', '\''Abstract 24x36'\'', 1702419200000)"'
sql '"INSERT INTO products VALUES (30, '\''Yoga Mat'\'', 29.99, 130, '\''Fitness'\'', '\''Non-slip 6mm'\'', 1702505600000)"'
echo "  30 products"

# Orders (35)
for i in $(seq 1 35); do
  UUID=$((RANDOM % 20 + 1))
  PID=$((RANDOM % 30 + 1))
  QTY=$((RANDOM % 5 + 1))
  PRICE=$(python3 -c "print(round($RANDOM / 100 + 5, 2))")
  TOTAL=$(python3 -c "print(round($QTY * $PRICE, 2))")
  STATUSES=("pending" "confirmed" "shipped" "delivered" "cancelled")
  STATUS="${STATUSES[$((RANDOM % 5))]}"
  TS=$((1703000000000 + i * 3600000 + RANDOM))
  ADDR="$((RANDOM % 9999 + 1)) Main St"
  sql "\"INSERT INTO orders VALUES ($i, $UUID, $PID, $QTY, $TOTAL, '${STATUS}', '${ADDR}', $TS)\""
done
echo "  35 orders"

# Logs (50)
LEVELS=("info" "warn" "error" "debug")
SOURCES=("auth" "cache" "sql" "http" "queue" "search" "blob" "memory" "scheduler")
MSGS=(
  "User logged in successfully"
  "Failed login attempt"
  "Cache hit for key user_session"
  "Cache miss for key product_list"
  "Database query completed"
  "Slow query detected (2.3s)"
  "Request to /api/products completed"
  "Queue message processed"
  "Email notification sent"
  "Search index refreshed"
  "Blob upload completed"
  "Job scheduled: cleanup_tmp"
  "Memory pressure warning (78%)"
  "Rate limit exceeded"
  "WebSocket connection opened"
  "File upload validation failed"
  "Background task completed"
  "Configuration reloaded"
)
for i in $(seq 1 50); do
  LVL="${LEVELS[$((RANDOM % 4))]}"
  [ $i -le 3 ] && LVL="error"
  SRC="${SOURCES[$((RANDOM % ${#SOURCES[@]}))]}"
  MSG="${MSGS[$((RANDOM % ${#MSGS[@]}))]}"
  TS=$((1704000000000 + i * 60000 + RANDOM))
  sql "\"INSERT INTO logs VALUES ($i, '${LVL}', '${MSG}', '${SRC}', ${TS})\""
done
echo "  50 log entries"

# Events (25)
EVT_NAMES=("page_view" "signup" "purchase" "search_query" "share_social" "add_to_cart" "view_product" "logout" "password_reset" "api_call" "export_data" "import_csv" "generate_report" "change_settings" "delete_account")
EVT_CATS=("navigation" "auth" "ecommerce" "search" "engagement" "ecommerce" "ecommerce" "auth" "auth" "integration" "data" "data" "analytics" "preferences" "auth")
for i in $(seq 1 25); do
  IDX=$((RANDOM % ${#EVT_NAMES[@]}))
  EV="${EVT_NAMES[$IDX]}"
  CAT="${EVT_CATS[$IDX]}"
  PAYLOAD="{\\\"uid\\\":$((RANDOM % 20 + 1))}"
  TS=$((1705000000000 + i * 1800000 + RANDOM))
  sql "\"INSERT INTO events VALUES ($i, '${EV}', '${CAT}', '${PAYLOAD}', ${TS})\""
done
echo "  25 events"
echo ""

# --- CACHE ---
echo "--- Cache ---"
curl -sf -X POST "$BASE/cache/batch" -H "$AUTH" -H 'Content-Type: application/json' \
  -d '[
    {"key":"config:site_name","value":"Nova Runtime Store"},
    {"key":"config:currency","value":"USD"},
    {"key":"config:locale","value":"en_US","ttl_ms":3600000},
    {"key":"feature:dark_mode","value":true},
    {"key":"rate_limit:api_default","value":{"requests":100,"per_seconds":60},"ttl_ms":300000},
    {"key":"rate_limit:auth","value":{"requests":5,"per_seconds":60},"ttl_ms":300000},
    {"key":"ui:theme","value":{"primary":"#4f46e5","secondary":"#7c3aed"}},
    {"key":"product:featured_ids","value":[3,7,12,18,25],"ttl_ms":86400000},
    {"key":"announcement:header","value":"Welcome to Nova!"},
    {"key":"db:schema_version","value":42}
  ]' > /dev/null
echo "  10 cache keys"
echo ""

# --- QUEUES ---
echo "--- Queues ---"
for q in email_notifications order_processing background_jobs webhook_delivery log_aggregation; do
  curl -sf -X POST "$BASE/queues" -H "$AUTH" -H 'Content-Type: application/json' \
    -d "{\"name\":\"$q\",\"durable\":true}" > /dev/null
  echo "  Queue: $q"
done

for i in $(seq 1 8); do
  curl -sf -X POST "$BASE/queues/email_notifications/messages" -H "$AUTH" -H 'Content-Type: application/json' \
    -d "{\"messages\":[{\"body\":{\"to\":\"user${i}@example.com\",\"subject\":\"Welcome!\",\"template\":\"welcome\"}}]}" > /dev/null
done
echo "  8 email messages"

for i in $(seq 1 6); do
  curl -sf -X POST "$BASE/queues/order_processing/messages" -H "$AUTH" -H 'Content-Type: application/json' \
    -d "{\"messages\":[{\"body\":{\"order_id\":$i,\"action\":\"process_payment\",\"amount\":$((RANDOM % 20000 + 500))}}]}" > /dev/null
done
echo "  6 order messages"

for i in $(seq 1 5); do
  curl -sf -X POST "$BASE/queues/background_jobs/messages" -H "$AUTH" -H 'Content-Type: application/json' \
    -d "{\"messages\":[{\"body\":{\"job\":\"report_$i\",\"type\":\"daily_sales\"}}]}" > /dev/null
done
echo "  5 background jobs"

for i in $(seq 1 4); do
  curl -sf -X POST "$BASE/queues/webhook_delivery/messages" -H "$AUTH" -H 'Content-Type: application/json' \
    -d "{\"messages\":[{\"body\":{\"url\":\"https://hooks.example.com/e${i}\",\"event\":\"order.created\"}}]}" > /dev/null
done
echo "  4 webhook messages"

for i in $(seq 1 6); do
  curl -sf -X POST "$BASE/queues/log_aggregation/messages" -H "$AUTH" -H 'Content-Type: application/json' \
    -d "{\"messages\":[{\"body\":{\"source\":\"api\",\"level\":\"info\",\"msg\":\"processed in ${i}ms\"}}]}" > /dev/null
done
echo "  6 log messages"
echo ""

# --- SCHEDULER ---
echo "--- Scheduler ---"
SCHED_JOBS=(
  '{"name":"cleanup_temp","type":"cron","schedule":"0 3 * * *","max_retries":3}'
  '{"name":"daily_report","type":"cron","schedule":"0 6 * * *","max_retries":2}'
  '{"name":"sync_external","type":"cron","schedule":"*/30 * * * *","max_retries":3}'
  '{"name":"cache_warm","type":"interval","max_retries":1}'
  '{"name":"health_check","type":"interval","max_retries":1}'
  '{"name":"send_newsletter","type":"cron","schedule":"0 9 * * 1","max_retries":3}'
  '{"name":"db_backup","type":"cron","schedule":"0 2 * * 0","max_retries":5}'
  '{"name":"process_refunds","type":"interval","max_retries":3}'
  '{"name":"update_search","type":"cron","schedule":"*/15 * * * *","max_retries":2}'
  '{"name":"expire_sessions","type":"interval","max_retries":1}'
)
for j in "${SCHED_JOBS[@]}"; do
  curl -sf -X POST "$BASE/scheduler/jobs" -H "$AUTH" -H 'Content-Type: application/json' \
    -d "$j" > /dev/null && echo "  Job: $(echo "$j" | python3 -c "import sys,json; print(json.load(sys.stdin)['name'])")"
done
echo ""

# --- SEARCH ---
echo "--- Search ---"
curl -sf -X POST "$BASE/search/indexes" -H "$AUTH" -H 'Content-Type: application/json' \
  -d '{"name":"products","fields":[{"name":"name","type":"text"},{"name":"category","type":"text"},{"name":"price","type":"float"}]}' > /dev/null
echo "  Index: products"

DOCS='{"documents":['
for i in $(seq 1 15); do
  [ $i -gt 1 ] && DOCS="$DOCS,"
  DOCS="${DOCS}{\"id\":\"prod_$i\",\"name\":\"Product $i\",\"category\":\"general\",\"price\":$((RANDOM % 10000 + 999))}"
done
DOCS="$DOCS]}"
curl -sf -X POST "$BASE/search/indexes/products/documents" -H "$AUTH" -H 'Content-Type: application/json' \
  -d "$DOCS" > /dev/null
echo "  15 product docs"

curl -sf -X POST "$BASE/search/indexes" -H "$AUTH" -H 'Content-Type: application/json' \
  -d '{"name":"articles","fields":[{"name":"title","type":"text"},{"name":"content","type":"text"},{"name":"author","type":"text"}]}' > /dev/null
echo "  Index: articles"

ADOCS='{"documents":['
ATITLES=("Getting Started" "Advanced Config" "API Reference" "Performance Tips" "Security Guide" "Deploy Guide" "Real-time Features" "Plugin Dev" "Data Migration" "Monitoring")
AAUTHORS=("Alice" "Bob" "Charlie" "Diana" "Eve" "Frank" "Grace" "Hank" "Ivy" "Jack")
for i in $(seq 0 9); do
  [ $i -gt 0 ] && ADOCS="$ADOCS,"
  ADOCS="${ADOCS}{\"id\":\"art_$(($i+1))\",\"title\":\"${ATITLES[$i]}\",\"content\":\"Guide for ${ATITLES[$i]}\",\"author\":\"${AAUTHORS[$i]}\"}"
done
ADOCS="$ADOCS]}"
curl -sf -X POST "$BASE/search/indexes/articles/documents" -H "$AUTH" -H 'Content-Type: application/json' \
  -d "$ADOCS" > /dev/null
echo "  10 article docs"
echo ""

# --- BLOBS ---
echo "--- Blobs ---"
echo "Welcome to Nova Runtime - your application is running." \
  | curl -sf -X POST "$BASE/blobs" -H "$AUTH" -H 'Content-Type: text/plain' --data-binary @- > /dev/null \
  && echo "  welcome.txt"
echo '{"app":"Nova Runtime","version":"0.1.0","features":["sql","cache","queues","scheduler","search","blobs"]}' \
  | curl -sf -X POST "$BASE/blobs" -H "$AUTH" -H 'Content-Type: application/json' --data-binary @- > /dev/null \
  && echo "  config.json"
echo '<h1>Nova Runtime</h1><p>Your app is running.</p>' \
  | curl -sf -X POST "$BASE/blobs" -H "$AUTH" -H 'Content-Type: text/html' --data-binary @- > /dev/null \
  && echo "  index.html"
python3 -c "
for i in range(1,11):
    print(f'{i},user{i},user{i}@example.com,member')
" | curl -sf -X POST "$BASE/blobs" -H "$AUTH" -H 'Content-Type: text/csv' --data-binary @- > /dev/null \
  && echo "  users.csv"
python3 -c "
import random
words = ['lorem','ipsum','dolor','sit','amet','consectetur','elit']
for _ in range(30):
    print(' '.join(random.choice(words) for _ in range(random.randint(5,12))).capitalize() + '.')
" | curl -sf -X POST "$BASE/blobs" -H "$AUTH" -H 'Content-Type: text/plain' --data-binary @- > /dev/null \
  && echo "  sample.txt (30 lines)"
echo 'host: 0.0.0.0
port: 8642
tls: false
db:
  path: /var/lib/nova
  max_conn: 100
log:
  level: info
  format: json' \
  | curl -sf -X POST "$BASE/blobs" -H "$AUTH" -H 'Content-Type: application/x-yaml' --data-binary @- > /dev/null \
  && echo "  config.yaml"
echo '<?xml version="1.0"?><nova><version>0.1.0</version><modules><m>sql</m><m>cache</m></modules></nova>' \
  | curl -sf -X POST "$BASE/blobs" -H "$AUTH" -H 'Content-Type: application/xml' --data-binary @- > /dev/null \
  && echo "  config.xml"
python3 -c "
for i in range(1,21):
    print(f'-- backup record {i}')
    print(f'INSERT INTO backup VALUES ({i}, sample_{i});')
" | curl -sf -X POST "$BASE/blobs" -H "$AUTH" -H 'Content-Type: application/sql' --data-binary @- > /dev/null \
  && echo "  dump.sql"
echo "  8 blobs"
echo ""

# --- AUTH ---
echo "--- Auth ---"
curl -sf -X POST "$BASE/auth/users" -H "$AUTH" -H 'Content-Type: application/json' \
  -d '{"username":"operator1","password":"Oper8!pass","roles":["operator"]}' > /dev/null && echo "  User: operator1"
curl -sf -X POST "$BASE/auth/users" -H "$AUTH" -H 'Content-Type: application/json' \
  -d '{"username":"viewer1","password":"View3r!pass","roles":["viewer"]}' > /dev/null && echo "  User: viewer1"
curl -sf -X POST "$BASE/auth/api-keys" -H "$AUTH" -H 'Content-Type: application/json' \
  -d '{"name":"ci-cd","permissions":["deploy","read"]}' > /dev/null && echo "  API key: ci-cd"
curl -sf -X POST "$BASE/auth/api-keys" -H "$AUTH" -H 'Content-Type: application/json' \
  -d '{"name":"monitoring","permissions":["read"]}' > /dev/null && echo "  API key: monitoring"
echo ""

echo "=== Done ==="
echo "SQL: 5 tables, 160 rows"
echo "Cache: 10 keys"
echo "Queues: 5 queues, 29 messages"
echo "Scheduler: 10 jobs"
echo "Search: 2 indexes, 25 documents"
echo "Blobs: 8 blobs"
echo "Auth: 2 users, 2 API keys"
