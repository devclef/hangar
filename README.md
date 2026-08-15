# Hangar — RC Hobby Inventory Tracker

A self-hosted, single-user web app for tracking RC models (helis, planes, cars, drones, boats)
and the parts/spares you own for them. The whole point: ask "do I have rotor blades for my
Kraken 580?" and get an answer in one click, including how many are on hand.

- **Backend:** Rust — [axum](https://github.com/tokio-rs/axum) HTTP API + [sqlx](https://github.com/launchbadge/sqlx) over SQLite, data access behind a repository trait so Postgres can slot in later without touching API or service code.
- **Frontend:** Svelte 5 + Vite + TypeScript + Tailwind CSS 4. Hash-routed SPA, no framework state libraries. Served as static files by the backend — one container, one port.
- **Database:** SQLite in a single file (WAL mode), schema tracked with sqlx migrations under `migrations/`.

## Quick start (Docker)

```bash
docker compose up --build
```

Open http://localhost:8080. The SQLite database lives in the `hangar-data` volume at `/data/hangar.db`,
so it survives container rebuilds. Stop with `docker compose down` (add `-v` to also wipe the DB).

## Run locally (dev)

Requires Rust (stable) and Node 20+.

```bash
# terminal 1 — backend on :8080, DB in ./data/
cargo run

# terminal 2 — frontend dev server on :5173 (proxies /api to :8080)
cd frontend
npm install
npm run dev
```

Use http://localhost:5173 during development. To serve the production-style single-port setup
without Docker, build the frontend and point the backend at it:

```bash
cd frontend && npm install && npm run build && cd ..
STATIC_DIR=frontend/dist cargo run   # then open http://localhost:8080
```

### Configuration (environment variables)

| Variable      | Default     | Description                                            |
| ------------- | ----------- | ------------------------------------------------------ |
| `PORT`        | `8080`      | Listen port.                                           |
| `DATA_DIR`    | `./data`    | Directory for the SQLite file (created if missing).    |
| `STATIC_DIR`  | `./static`  | Frontend build output to serve (set to `frontend/dist` in local prod mode, `/app/static` in Docker). |
| `DATABASE_URL`| `sqlite://$DATA_DIR/hangar.db?mode=rwc` | Override the database location.   |
| `RUST_LOG`    | `hangar=info,tower_http=info` | Logging filter (e.g. `debug`).        |

### Tests

```bash
cargo test          # unit tests + end-to-end API tests (in-memory SQLite)
cd frontend && npm run check   # svelte-check
```

## API

All endpoints are under `/api` and use JSON. Errors have the shape
`{"error": "<code>", "message": "<human readable>"}` with `400 invalid_request`,
`404 not_found`, or `500 internal`.

### Health

| Method | Path             | Description       |
| ------ | ---------------- | ----------------- |
| GET    | `/api/health`    | Liveness probe.   |

### Models

| Method | Path                     | Description |
| ------ | ------------------------ | ----------- |
| GET    | `/api/models`            | List models. Query params: `q` (name/manufacturer/notes substring), `category` (`heli\|plane\|car\|drone\|boat\|other`). Each row includes `part_count`. |
| POST   | `/api/models`            | Create a model. Body: `name`*, `category`*, `manufacturer?`, `notes?`, `date_acquired?` (YYYY-MM-DD), `status?` (`active\|retired\|sold`, default `active`), `photo_url?`. Returns 201. |
| GET    | `/api/models/:id`        | Model detail: `{model, parts[]}` where `parts` are the linked parts with live quantities. This is the "what do I have for this model?" endpoint. |
| PUT    | `/api/models/:id`        | Full replace update (same body as create). |
| DELETE | `/api/models/:id`        | Delete model (its links are removed; parts remain in inventory). |

### Model ↔ part links

| Method | Path                              | Description |
| ------ | --------------------------------- | ----------- |
| GET    | `/api/models/:id/parts`           | Linked parts (with quantities). |
| POST   | `/api/models/:id/parts`           | Link one part. Body: `{"part_id": 3}`. Idempotent (204). |
| PUT    | `/api/models/:id/parts`           | Replace the full link set. Body: `{"part_ids": [3, 5]}`. 404 if any part id is unknown (set is left unchanged). |
| DELETE | `/api/models/:id/parts/:part_id`  | Unlink one part. 404 if not linked. |

### Parts

| Method | Path                       | Description |
| ------ | -------------------------- | ----------- |
| GET    | `/api/parts`               | List parts. Query params: `q` (name/notes/link substring), `sort` (`quantity_asc` default-friendly, `quantity_desc`, `name_asc`, `name_desc`, `recent`). Each row includes `model_count` and `model_names`. |
| POST   | `/api/parts`               | Create a part. Body: `name`*, `quantity`* (≥ 0), `notes?`, `link?` (URL or SKU), `photo_url?`, `cost?` (≥ 0, per unit in the configured currency), `vendor?`. Returns 201. |
| GET    | `/api/parts/:id`           | Part detail: `{part, models[]}` — all compatible models. |
| PUT    | `/api/parts/:id`           | Full replace update (same body as create). |
| DELETE | `/api/parts/:id`           | Delete part (unlinked from all models). |
| POST   | `/api/parts/:id/quantity`   | Atomic relative change: `{"delta": -1}`. Clamps at 0, so you can never go negative. Returns the updated part. |
| GET    | `/api/parts/:id/models`     | Models linked to this part. |
| POST   | `/api/parts/:id/models`     | Link one model. Body: `{"model_id": 1}`. Idempotent (204). |
| DELETE | `/api/parts/:id/models/:model_id` | Unlink one model. 404 if not linked. |

### Part usage log

A log of parts consumed against models (repairs, builds, swaps). Recording a
usage also decrements the part's stock by the logged quantity (clamped at 0,
same rule as quantity adjusts), in one transaction. Entries are append-only
and cascade when their part or model is deleted.

| Method | Path                     | Description |
| ------ | ------------------------ | ----------- |
| GET    | `/api/usage`             | Log entries, newest first. Query params: `part_id`, `model_id` (either may be omitted for "any"). Each row includes `part_name`, `model_name`, and `model_category` so the log is self-describing. |
| POST   | `/api/parts/:id/usage`   | Record usage of this part. Body: `model_id`*, `quantity?` (≥ 1, default 1), `notes?` (e.g. "replaced pitch rods"), `used_at?` (`YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SS`; defaults to now). Returns 201 with the entry. |
| POST   | `/api/models/:id/usage`  | Record usage on this model. Body: `part_id`* plus the same optional fields as above. Returns 201 with the entry. |

### Settings

| Method | Path           | Description |
| ------ | -------------- | ----------- |
| GET    | `/api/settings`| Current settings. Returns `{"part_form_fields": [...], "currency": "USD"}` — the defaults (all fields, USD) when nothing is stored yet. |
| PUT    | `/api/settings`| Full replace of the settings document (same shape as GET). `part_form_fields` is a list drawn from `quantity, cost, vendor, link, photo_url, notes` (duplicates collapsed, unknown values are a 400). `currency` is an ISO-4217 code, normalized to uppercase (3-8 alphanumeric characters). |

Example — the question this app exists for:

```bash
curl -s localhost:8080/api/models/1
# → { "model": { "name": "Kraken 580", ... },
#     "parts": [ { "name": "Main rotor blade set", "quantity": 2, ... }, ... ] }
```

## Frontend pages

- `#/models` — list, search, category filter chips, linked-part counts.
- `#/models/:id` — model detail: all linked parts with quantities (inline +/− stepper), link/unlink parts.
- `#/models/new`, `#/models/:id/edit` — add/edit forms.
- `#/parts` — all parts, searchable, sortable (defaults to quantity low→high so out-of-stock floats to the top; 0 shows an "out" badge, ≤2 a "low" badge).
- `#/parts/:id` — part detail: quantity stepper, compatible models, link/unlink models.
- `#/parts/new`, `#/parts/:id/edit` — add/edit forms.
- `#/usage` — usage log: every part used on every model, with when/quantity/notes, filterable by part or model, plus a "log a usage" form.
- `#/models/:id` and `#/parts/:id` also show a "recent usage" card with an inline log form for that model/part.
- `#/settings` — pick which fields the part form shows (toggles, saved to the API) and the currency code used to display part costs.

## Decisions & Assumptions

Things the brief left open and how they were resolved:

- **Photos:** the `photo_url` field exists on both models and parts (a URL or server-relative path) and renders on detail pages, but v1 has **no file-upload endpoint** — that keeps the API surface small. Drop files into a served directory (or point the field at any reachable URL) and it just works; an upload endpoint can be added later without schema changes.
- **Category:** model `category` is a fixed enum (`heli, plane, car, drone, boat, other`) so filtering is simple.
- **Part type removed:** parts once had a free-text `part_type`, but with no controlled vocabulary it fragmented into inconsistent filter values and blurred into the name; it was dropped (migration `0004` removes the column and its stored values). Name + substring search covers the real use cases.
- **Part form fields:** the add/edit part form always shows the required name; every other field (quantity, cost, vendor, link/SKU, photo, notes) can be hidden from the form in `#/settings`. Hidden fields still exist on the record — edits just don't surface them — so nothing is ever wiped by hiding a field. The choice is stored server-side (`settings` table) and the part form reads it on load.
- **Cost & vendor:** `cost` is a nullable `REAL` (per-unit, in the user's configured currency; validated finite and ≥ 0) and `vendor` is a nullable free-text string. Costs render with `Intl.NumberFormat` using the `currency` setting (ISO-4217, default USD), falling back to a plain `$` formatting if the runtime doesn't support the code.
- **Link/SKU:** one string field `link` on parts. If it looks like an http(s) URL the UI renders it as a link, otherwise as a monospace SKU.
- **Update semantics:** `PUT` is a full replace of the record (the forms always submit everything), which avoids sparse-patch edge cases in a single-user tool. Quantity changes also have the atomic `POST .../quantity {delta}` endpoint, clamped at 0 server-side.
- **Association endpoints:** both sides of the M:N are manageable (`/models/:id/parts` and `/parts/:id/models`); the model side also supports full-set replace. Duplicate links are idempotent no-ops.
- **Usage log:** a "usage" is a part consumed on a model. Recording one decrements stock by the same amount (clamped at 0) so the drawer count and the log always agree; if you log more than is on hand the entry keeps the real quantity and the count clamps to 0. Entries are append-only: a mistaken entry is corrected by adjusting stock, not by deleting history. `model_id` is required on every entry because the whole point of the log is "which model did this go into?".
- **Sorting/searching:** case-insensitive substring search over the obvious text fields (name/manufacturer/notes for models, name/notes/link for parts); `LIKE` wildcards in user input are escaped.
- **Dates:** `date_acquired` is stored as an ISO-8601 date string, validated calendar-correctly (leap years included).
- **Static serving:** the backend serves the built SPA itself (hashed assets cached immutably, `index.html` no-cache) and returns JSON 404s for unknown `/api/*` paths. The SPA uses hash routing, so no server-side route fallback is needed.
- **Single process, no auth:** per the brief — one user, trusted network. SQLite pool is single-connection (WAL) which fits that profile and avoids `SQLITE_BUSY` entirely.
- **Postgres path:** everything below the HTTP handlers goes through `HangarRepo`/`ServiceApi` traits; a Postgres implementation would be a new module plus a feature flag, no route/service changes.
- **Error contract:** deserialization failures (bad JSON, unknown enum values, bad path params) are mapped to the same structured 400 responses as domain validation, so clients can rely on one error shape.

## Project layout

```
├── Cargo.toml            # backend crate (bin `hangar` + lib for tests)
├── migrations/           # sqlx migrations (0001_init.sql)
├── src/
│   ├── main.rs           # boot: env, pool, migrations, serve
│   ├── lib.rs            # library root
│   ├── routes.rs         # axum router + handlers (thin)
│   ├── service.rs        # business rules behind ServiceApi trait
│   ├── repo/             # HangarRepo trait + SqliteRepo
│   ├── types.rs          # domain types, inputs, validation
│   ├── error.rs          # DomainError → JSON error responses
│   └── web.rs            # static SPA serving + API 404s
├── tests/api.rs          # end-to-end API tests (in-memory SQLite)
├── frontend/             # Svelte 5 + Vite + TS + Tailwind SPA
├── Dockerfile            # multi-stage: node build → rust build → slim runtime
└── docker-compose.yml    # single service + persistent hangar-data volume
```
