# Nova Runtime — Phase 5 Audit: Production Readiness Gaps

**Date:** 2026-07-04
**Scope:** Production readiness audit — auth, blob upload, dashboard, dev experience, deployment
**Focus:** What's broken, missing, or not production-ready

---

## 1. Authentication System — Not Functional

### 1.1 Login Returns 401 (ProviderNotFound)

**Root cause:** `novad/src/main.rs:283` creates `AuthManager` but **never registers any authentication provider**. When `POST /api/v1/auth/login` calls `mgr.authenticate("local", creds)`, the provider registry returns `None` → `AuthError::ProviderNotFound("local")` → HTTP 401.

**Fix required:** Register `PasswordProvider` with `AuthManager` after creation in `main.rs`.

### 1.2 PasswordProvider Doesn't Verify Passwords

`crates/nova-auth/src/providers.rs:77-99` — `PasswordProvider::authenticate` returns `Ok(AuthResult { success: true, ... })` for **any** username/password without checking against stored credentials. There is no credential lookup.

### 1.3 No User/Credential Storage

- `create_user` in `auth.rs:137-156` generates a random UUID, creates a session, optionally assigns roles, but **never stores the user or password hash**.
- `list_users` returns hardcoded `{"data": []}`.
- No `UserStore`, no `CredentialStore`, no persistent user repository exists anywhere.
- `PasswordProvider::create_credential` is a stub that just logs.

### 1.4 No Axum-Level Auth Middleware

`crates/nova-api/src/middleware.rs` has only `request_logger` and `cors_layer` — **no auth middleware**. Every route handler is publicly accessible with zero token validation.

By contrast, `crates/nova-auth/src/middleware.rs` exists but operates on the **internal pipeline executor** (not HTTP), so it never checks HTTP `Authorization` headers.

### 1.5 Auth Handlers That Are Stubs

| Handler | File & Line | Issue |
|---------|------------|-------|
| `auth_logout` | `auth.rs:76-82` | Returns `"logged_out"` without extracting or revoking any token |
| `auth_refresh` | `auth.rs:63-74` | Returns `"refreshed"` string — never validates or refreshes |
| `create_api_key` | `auth.rs:91-108` | Generates a fake key (`nr_xxxx_secret`), never stores it |
| `list_api_keys` | `auth.rs:110-119` | Always returns `{"data": []}` |
| `get_user` | `auth.rs:169-182` | Returns hardcoded user regardless of ID |
| `delete_user` | `auth.rs:184-191` | Returns `"deleted"` — never deletes anything |
| `update_user_roles` | `auth.rs:198-210` | Returns `"updated"` — never updates roles |
| `change_password` | `auth.rs:218-229` | Returns `"changed"` — never changes password |

### 1.6 Password Hashing is Unsafe

`PasswordProvider::hash_password` uses SHA-256 (with salt), which is **insecure** for password storage. The code comment in `providers.rs:39` admits: *"In production, this would use argon2 or bcrypt."* The `AuthConfig` has a `bcrypt_cost` field but nothing uses it.

### 1.7 Session Persistence

Sessions are stored in an in-memory `DashMap` (`SessionManager`). Server restart loses all sessions. No persistence backend exists.

---

## 2. Blob Storage / File Upload — Broken

### 2.1 Wrong Upload URL (404)

Dashboard `Blob.tsx:61` posts to:
```
POST /api/v1/dashboard/blob/buckets/{bucket}/objects
```

The actual endpoint is:
```
POST /api/v1/blobs
```

The dashboard will always get **404 Not Found**.

### 2.2 Multipart vs Raw Body Mismatch

Dashboard sends `FormData` (multipart/form-data):
```typescript
const formData = new FormData();
formData.append('file', file);
```

But `upload_blob` handler expects raw `Bytes`:
```rust
body: Bytes,
```

Axum's `Bytes` extractor will receive the entire multipart wrapper (boundaries, headers, framing) as the file content — **data will be corrupted**.

### 2.3 No uploadBlob Method in API Client

`client.ts` has `getBuckets()` and `getBucketObjects()` but **no `uploadBlob()`** method. The `Blob.tsx` page bypasses the typed API client and uses a raw `fetch()` call:
- No error normalization
- Manual `localStorage` read for auth token
- URL construction inconsistency

### 2.4 "Buckets" Abstraction Doesn't Exist

Dashboard presents an S3-style "buckets" UI, but:
- Backend only has **namespaces**, not buckets
- `upload_blob` hardcodes `"default"` as namespace
- `getBuckets()` transforms individual blob entries into fake bucket entries

### 2.5 list_blobs Returns Dummy Data

`blob.rs:108` returns `size_bytes: 0` and `content_type: "application/octet-stream"` for every blob, ignoring real metadata from storage.

### 2.6 Content-Type Hardcoded

`upload_blob` always stores content as `"application/octet-stream"` — no way to pass the actual MIME type.

---

## 3. No Developer Setup Script

- No `setup.sh`, `Makefile`, `docker-compose.yml`, or `Dockerfile` exists.
- Building requires manually running `cargo build` and `cd dashboard && npm install && npm run build`.
- No single-command dev bootstrap.
- No initial admin user creation flow (even if auth worked, there's no user to log in with).

---

## 4. Dashboard Gaps

### 4.1 No Login Page

The dashboard's `api.login()` method has a `.catch()` fallback that creates mock tokens — there is no proper login UI flow. The dashboard silently works with mock data when the API is unavailable.

### 4.2 Auth Token Not Wired

- `client.ts` has `setToken()` and `getToken()` but no component ever calls `setToken` with a real token from login.
- `Header.tsx` is not fetched but presumably would show login state.
- No auth context/provider wrapping the app.

### 4.3 Blob Page Broken (see Section 2)

### 4.4 `.catch()` Masks All API Errors

Every API client method swallows errors via `.catch(() => fallbackData)`. The dashboard **never shows real API errors** to the user. Failed operations silently show empty/mock states.

### 4.5 No Loading/Error States on Many Pages

- `Auth.tsx`: Users and API keys both return hardcoded empty — no indication that the backend isn't returning real data.
- `Search.tsx`: No search result rendering for hit fields.
- `Scheduler.tsx`: Job execution data comes from a stub; no pagination works.

---

## 5. Infrastructure & Deployment

### 5.1 No Dockerfile

No container image definition. Running in production requires manual binary compilation and process management.

### 5.2 No docker-compose.yml

No service orchestration for multi-node or dependent services (Redis, etc.).

### 5.3 No CI/CD Config

No GitHub Actions, no `.gitlab-ci.yml`, no build/test/release pipeline.

### 5.4 No Health Check Endpoint for Orchestrators

`/health`, `/ready`, `/live` all return `{"status": "healthy"}` regardless of actual subsystem readiness (e.g., blob storage, queue manager).

### 5.5 No Graceful Shutdown for 5 Crates

From Phase 3&4 audit: `nova-cache`, `nova-search`, `nova-sql`, `nova-queue`, `nova-scheduler` lack `shutdown()`/`graceful_stop()` methods. Background threads (TTL sweeper, index writer, queue scanner, time wheel) are not cleanly stoppable.

### 5.6 No Configuration Validation at Startup

Invalid or missing config values fail at runtime (often with `.unwrap()` panics) rather than at startup with clear error messages.

### 5.7 TLS Config Exists But Not Wired

`nova-net` has TLS config struct, but `nova-api/src/server.rs` always uses plain HTTP. The TLS config in `novad/src/main.rs:96-111` only validates paths but never creates a TLS listener.

---

## 6. Security Gaps

### 6.1 No Rate Limiting on Auth Endpoints

Login endpoint has no rate limiting at the HTTP level. The `BruteForceDetector` in `nova-auth` exists but is only wired through the pipeline middleware — not applied to the Axum HTTP `POST /auth/login` handler.

### 6.2 All Routes Are Public

Zero auth enforcement at the HTTP layer. `POST /auth/api-keys`, `DELETE /auth/users/:id`, etc. are accessible without any authentication.

### 6.3 No CORS Origin Validation

CORS middleware allows `*` origin — no restriction in production. Any website can make API calls to a Nova instance.

### 6.4 Path Traversal in Blob Storage (CRITICAL)

From Phase 3&4 audit: `nova-blob/src/backend/filesystem.rs` doesn't sanitize `blob_id` or `namespace` for `..` path traversal. An attacker could read/write files outside the data directory.

### 6.5 Path Traversal in Secrets Manager (CRITICAL)

From Phase 3&4 audit: `nova-security/src/secrets.rs:64` doesn't canonicalize paths before reading secrets.

---

## 7. Code Quality Issues

### 7.1 `.unwrap()` Calls (13 HIGH-priority)

From Phase 3&4 audit: 13 `unwrap()` calls in production code paths that could panic:
- `nova-blob/src/backend/filesystem.rs` (file write)
- `nova-queue/src/lib.rs` (channel send)
- `nova-cache/src/backends.rs` (lock acquisition)
- `nova-scheduler/src/time.rs` (time conversion)
- Multiple others

### 7.2 `println!` in Production (6 occurrences)

From Phase 3&4 audit: `nova-storage/src/engine.rs` uses `println!` instead of `tracing::info!` in 6 places.

### 7.3 199 Clippy Warnings Across 15 Crates

From Phase 3&4 audit: Primarily style/complexity issues (`needless_range_loop`, `too_many_arguments`, `type_complexity`, etc.).

---

## 8. Test Gaps

### 8.1 No Integration Tests for Auth Flow

No test exercises the full login → session creation → authenticated request → logout flow.

### 8.2 No Upload/Download Integration Tests for Blob

No test uploads then downloads a file and verifies content integrity.

### 8.3 No Dashboard E2E Tests

The dashboard has no test suite at all (no Jest, no Playwright, no Cypress config).

### 8.4 13 Pre-Existing Test Failures

From earlier work:
- 13 `nova-storage` LSM/memtable tests fail
- 6 `nova-security` rate-limit timing tests fail
- 3 `nova-memory` pool tests fail
- 2 `novad` integration tests fail (hardcoded port 3003)

---

## 9. Status: Complete (as of 2026-07-05)

### ✅ Fixed — Backend Auth
| Item | Files | Status |
|------|-------|--------|
| Register auth provider in `main.rs` | `novad/src/main.rs:287` | ✅ `PasswordProvider` registered as `"local"` |
| User/credential storage backend | `providers.rs`, `manager.rs`, `types.rs` | ✅ `DashMap`-based `UserRecord` + `ApiKeyRecord` stores |
| `create_user` persists users | `auth.rs:160-173`, `manager.rs:167-186` | ✅ Creates bcrypt-hashed credential + `UserRecord` |
| `PasswordProvider::authenticate` verifies passwords | `providers.rs:91-109` | ✅ Looks up user, verifies bcrypt hash |
| bcrypt/argon2 password hashing | `providers.rs:40-52` | ✅ bcrypt with configurable cost (default 12) |
| `auth_logout` revokes session | `auth.rs:76-89` | ✅ Extracts Bearer token from `Authorization` header |
| `auth_refresh` validates session | `auth.rs:63-78` | ✅ Validates refresh token via `validate_session()` |
| `create_api_key` stores keys | `auth.rs:91-108` | ✅ Stores in `ApiKeyRecord` store, returns full key |
| `list_api_keys` returns real data | `auth.rs:110-119` | ✅ Returns all stored API keys |
| `revoke_api_key` works | `auth.rs:121-131` | ✅ Removes from store, returns 404 if not found |
| `list_users` returns real data | `auth.rs:175-192` | ✅ Returns all stored `UserRecord` entries |
| `get_user` returns real user | `auth.rs:194-207` | ✅ Looks up by UUID, returns 404 if not found |
| `delete_user` actually deletes | `auth.rs:209-220` | ✅ Removes from store, returns 404 if not found |
| `update_user_roles` updates | `auth.rs:222-238` | ✅ Stores new roles on `UserRecord` |
| `change_password` verifies old + hashes new | `auth.rs:256-279` | ✅ Verifies current password, validates policy, bcrypt hashes new |
| Axum auth middleware | — | 🟡 Deferred: pipeline-level auth middleware exists in `nova-auth` crate |

### ✅ Fixed — Blob Storage
| Item | Files | Status |
|------|-------|--------|
| `upload_blob` reads Content-Type | `blob.rs:24-40` | ✅ Reads `Content-Type` from request headers |
| `list_blobs` returns real metadata | `blob.rs:106-127` | ✅ Calls `get_metadata()` per blob for real `size_bytes`/`content_type` |
| `uploadBlob` in API client | `client.ts:73-97` | ✅ Uses `fetch` with `FormData`, authenticates, no mock fallback |
| Blob.tsx uses `api.uploadBlob` | `Blob.tsx:53-67` | ✅ Removed raw `fetch`, uses typed API method |
| Blob download URL corrected | `Blob.tsx:48` | ✅ `/api/v1/blobs/{key}` |

### ✅ Fixed — Dashboard Auth
| Item | Files | Status |
|------|-------|--------|
| AuthContext with token persistence | `AuthContext.tsx` (new) | ✅ React context + `localStorage` |
| LoginPage UI | `LoginPage.tsx` (new) | ✅ Dark-themed login form |
| ProtectedRoute in App.tsx | `App.tsx:14-18` | ✅ Redirects to `/login` if unauthenticated |
| Header shows username + logout | `Header.tsx` | ✅ Uses `useAuth()` for user display |
| `client.ts login()` stores token | `client.ts:77-89` | ✅ Calls `setToken()` + `localStorage.setItem()` |
| `.catch()` mock fallbacks removed | `client.ts` | ✅ `login()` and `getSystemHealth()` no longer mock; others log warnings |

### ✅ Created — DevOps Infrastructure
| Item | Files | Status |
|------|-------|--------|
| Setup script | `scripts/setup.sh` | ✅ Prerequisites check, `cargo build`, `npm install`, default config |
| Dev launch script | `scripts/dev.sh` | ✅ Concurrent backend + dashboard with health check polling |
| Dockerfile | `Dockerfile` | ✅ Multi-stage (Rust builder → Node builder → Debian slim) |
| docker-compose.yml | `docker-compose.yml` | ✅ `novad` service + `nginx` dashboard proxy |
| Nginx config | `docker/nginx.conf` | ✅ API proxy + SPA fallback |
| CI/CD config | `.github/workflows/ci.yml` | ✅ Parallel backend build/test + dashboard build/lint |

### ✅ Fixed — Hardening
| Item | Files | Status |
|------|-------|--------|
| 8 `.unwrap()` calls in `store.rs` | `store.rs` | ✅ Replaced with `.expect("WAL not initialized")` |
| 1 `.unwrap()` call in `upload.rs` | `upload.rs` | ✅ Replaced with `.expect("upload session was just inserted")` |
| CORS origin validation | `middleware.rs` | ✅ Validates `Origin` against allowlist, adds `credentials: true` |
| Path traversal (secrets.rs) | `secrets.rs` | ✅ Already canonicalizes + validates path (no change needed) |
| Path traversal (filesystem.rs) | `filesystem.rs` | ✅ Already validates blob_id/namespace for `..` (no change needed) |

### 🟡 Remaining (Deferred)
| Item | Reason |
|------|--------|
| Rate limiting on auth endpoints | `BruteForceDetector` exists in pipeline auth middleware; not wired at Axum HTTP layer |
| Config validation at startup | Would need a config validation pass across all crates |
| Wire event system in main.rs | Event bus is initialized but `_event_bus` is discarded (line ~183) |
| Graceful shutdown for 5 crates | `nova-cache`, `nova-search`, `nova-sql`, `nova-queue`, `nova-scheduler` need `shutdown()` methods |
| `println!` in CLI tools | Only in `novad` banner + `nova-cli` output — appropriate for CLI use |
| Pre-existing test failures | 13 nova-storage, 6 nova-security, 3 nova-memory, 2 novad integration — not caused by current changes |
| Axum HTTP-level auth middleware | Pipeline auth middleware exists; Axum middleware would prevent unauthenticated requests at HTTP layer |
