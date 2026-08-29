# NovaBoard — Taskboard Demo 100% on Nova Runtime

A Trello-like Kanban board where **every byte lives in Nova** — no Postgres, no Redis, no S3.

| Nova Subsystem | What it stores |
|----------------|----------------|
| **SQL** | `users`, `projects`, `tasks`, `comments` tables — all queries via `POST /api/v1/sql/query` & `execute` |
| **Cache** | Hot tasks & `board:stats` with TTL |
| **Queue** | `task-notifications` FIFO + `task-reminders` delayed queue |
| **Scheduler** | `due-reminder` cron `*/5 * * * *` + one-shot due dates |
| **Search** | `tasks_idx` BM25 index for title/description |
| **Blob** | Task attachments via `POST /api/v1/blobs?namespace=taskboard` |
| **Auth** | `admin/admin123` + demo users, API keys, JWT |

## Prereqs

```bash
# 1. Nova must be running
make dev      # from repo root → http://127.0.0.1:8642 + dashboard 5173
# or
docker compose up --build
```

## Run Demo

```bash
cd examples/taskboard
npm install
npm run seed   # creates tables, 3 projects, 5 users, 24 tasks, cache, queue, scheduler, search index, blobs
npm run dev    # http://localhost:3000 — Kanban UI

# View same data in Nova Dashboard:
# http://127.0.0.1:5173 → Database (tasks table), Search (tasks_idx), Blob (taskboard namespace)
```

## Seed what?

`npm run seed` prints:

```
✓ SQL: users (5), projects (3), tasks (24), comments (12)
✓ Cache: board:stats (TTL 60s), hot:task:* (5)
✓ Queue: task-notifications (3 msgs), task-reminders (2 delayed)
✓ Scheduler: due-reminder (cron), task-due-7 (one-shot)
✓ Search: tasks_idx (24 docs)
✓ Blob: 3 attachments in taskboard namespace
✓ Auth: demo users alice/bob/carol
```

## API

Demo server at `:3000` proxies to Nova at `8642` and exposes:

- `GET /api/board` → aggregated board (SQL + cache)
- `POST /api/tasks` → SQL insert + cache invalidate + queue + search index
- `PUT /api/tasks/:id/move` → SQL update + event
- `POST /api/tasks/:id/attach` → Blob upload

All state is in Nova — restart Nova and data persists in `./data`.

## Clean

```bash
npm run clean  # DROP TABLE + purge queues/indexes/blobs
```
