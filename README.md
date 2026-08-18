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
| `CATALOG_DIR` | `./catalog-data`  | Directory of parts-catalog source files, imported into the DB at startup (and by `cargo run -- import-catalog`). |
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
| GET    | `/api/models/:id`        | Model detail: `{model, parts[]}` where `parts` are the linked parts with live quantities. This is the "what do I have for this model?" endpoint. `model` includes `catalog_model_id` (when linked), and a `catalog` summary (`catalog_model_name`, `diagram_asset`) is embedded so the detail page can show the "known parts / diagram" section without a second round trip. |
| PUT    | `/api/models/:id`        | Full replace update (same body as create). |
| DELETE | `/api/models/:id`        | Delete model (its links are removed; parts remain in inventory). |

### Model ↔ part links

| Method | Path                              | Description |
| ------ | --------------------------------- | ----------- |
| GET    | `/api/models/:id/parts`           | Linked parts (with quantities). |
| POST   | `/api/models/:id/parts`           | Link one part. Body: `{"part_id": 3}`. Idempotent (204). |
| PUT    | `/api/models/:id/parts`           | Replace the full link set. Body: `{"part_ids": [3, 5]}`. 404 if any part id is unknown (set is left unchanged). |
| DELETE | `/api/models/:id/parts/:part_id`  | Unlink one part. 404 if not linked. |

### Model ↔ catalog links

| Method | Path                          | Description |
| ------ | ----------------------------- | ----------- |
| POST   | `/api/models/:id/link-catalog` | Links (or re-points) a model to a reference catalog model. Body: `{"catalog_model_id": N}`. 404 for unknown ids; 400 when the categories don't match. Re-linking the same model is an idempotent no-op. Returns the updated model. |
| DELETE | `/api/models/:id/link-catalog` | Unlinks. 404 when the model has no catalog link. |

### Parts

| Method | Path                       | Description |
| ------ | -------------------------- | ----------- |
| GET    | `/api/parts`               | List parts. Query params: `q` (name/notes/link substring), `sort` (`quantity_asc` default-friendly, `quantity_desc`, `name_asc`, `name_desc`, `recent`). Each row includes `model_count` and `model_names`. |
| POST   | `/api/parts`               | Create a part. Body: `name`*, `quantity`* (≥ 0), `notes?`, `link?` (URL or SKU), `photo_url?`, `cost?` (≥ 0, per unit in the configured currency), `vendor?`, `low_stock_enabled?` (default `true`; `false` opts this part out of the "low" quantity badge). Returns 201. |
| POST   | `/api/parts/bulk-edit`     | Bulk-update several parts in one transaction. Body: `part_ids`* (1–500, dupes collapse) plus any of `quantity`, `cost`, `vendor`, `link`, `photo_url`, `notes`, `low_stock_enabled` — each **tri-state**: omitted keeps the value, `null` clears it, a value overwrites it — and `model_id` (link this model to every selected part, idempotent) / `unlink_model_ids` (unlink these models; absent links are no-ops). 404 if any part or model id is unknown, 400 when there is nothing to change. Returns the updated rows. |
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

### Parts catalog

Reference catalog of known manufacturer/model combinations and their official
parts, imported from the versioned files in `catalog-data/` (see
`catalog-data/README.md` for the file format and how to add a model). Catalog
rows are created/refreshed by the importer only — at startup and via
`cargo run -- import-catalog [path]` — not by request bodies.

| Method | Path                                        | Description |
| ------ | ------------------------------------------- | ----------- |
| GET    | `/api/catalog/manufacturers`                | List catalog manufacturers, each with `model_count`. |
| GET    | `/api/catalog/manufacturers/:id/models`     | Catalog models for a manufacturer. 404 for an unknown manufacturer. |
| GET    | `/api/catalog/models/:id`                   | Catalog model detail: `{model, diagram_asset, linked_models[], parts[]}`. Each part carries its diagram coordinates and `owned_quantity` — the live sum over the inventory parts tied to that catalog part (`parts.catalog_part_id`) and linked to the user models linked to this catalog model; `null` when no user model is linked. Optional query param `model_id` scopes the quantities to one specific user model (still `null` if that model isn't linked). |
| POST   | `/api/catalog/parts/:id/add-to-inventory`   | One-click add to a model's inventory. Body: `{"model_id": N, "quantity"?: number}`. Creates a `parts` row pre-filled from the catalog entry (`name`, `part_number` → the part's `link` field, `catalog_part_id` set) and links it to the model (201); if that catalog part is already tied to an inventory part on the model, adjusts that part's quantity by `quantity` instead (delta semantics, clamped at 0, default +1; 200). 404 for unknown ids, 400 for a zero/negative start on the create path. |
| DELETE | `/api/catalog/parts/:id`                    | Explicit admin deletion of a catalog part (orphan cleanup). Inventory parts keep existing; their `catalog_part_id` becomes `null`. 404 for an unknown id. |

### Settings

| Method | Path           | Description |
| ------ | -------------- | ----------- |
| GET    | `/api/settings`| Current settings. Returns `{"part_form_fields": [...], "currency": "USD", "low_stock_enabled": true, "low_stock_threshold": 2, "theme": "system"}` — the defaults (all fields, USD, low stock on at 2, system theme) when nothing is stored yet. |
| PUT    | `/api/settings`| Full replace of the settings document (same shape as GET). `part_form_fields` is a list drawn from `quantity, cost, vendor, link, photo_url, notes` (duplicates collapsed, unknown values are a 400). `currency` is an ISO-4217 code, normalized to uppercase (3-8 alphanumeric characters). `low_stock_enabled` globally switches the "low" quantity badge on/off; `low_stock_threshold` (0–1000) is the quantity at or below which a part counts as "low". `theme` is the default color mode: `system` (follow the OS), `light`, or `dark`; the UI's light/dark toggle persists here. |

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
- `#/catalog` — parts catalog: browse manufacturers → models (with their source files).
- `#/catalog/models/:id` — a catalog model's generic diagram with numbered, color-coded hotspot pins (gray = not in inventory, green = in stock, amber = low, red = out) plus a parts list showing part numbers, groups, live owned quantities, and a one-click "add to inventory" action (targets your model(s) linked to this catalog model).
- The model detail page also has a **Catalog** section: linked models show the same diagram + parts view scoped to that model's quantities (plus unlink); unlinked models get a "link to catalog model" picker filtered to matching categories.
- `#/parts` — all parts, searchable, sortable (defaults to quantity low→high so out-of-stock floats to the top; 0 shows an "out" badge, and at-or-below the configured low-stock threshold a "low" badge). Rows are checkbox-selectable; selecting one or more opens a **bulk edit** panel that changes any of the part fields (set or clear) on every selected part at once, and links/unlinks a chosen model across the selection.
- `#/parts/:id` — part detail: quantity stepper, compatible models, link/unlink models.
- `#/parts/new`, `#/parts/:id/edit` — add/edit forms.
- `#/usage` — usage log: every part used on every model, with when/quantity/notes, filterable by part or model, plus a "log a usage" form.
- `#/models/:id` and `#/parts/:id` also show a "recent usage" card with an inline log form for that model/part.
- `#/settings` — pick which fields the part form shows (toggles, saved to the API), the low-stock badge (global on/off + threshold), the default color theme (system/light/dark; the header's sun/moon button flips light/dark at any time), and the currency code used to display part costs.

## Decisions & Assumptions

Things the brief left open and how they were resolved:

- **Photos:** the `photo_url` field exists on both models and parts (a URL or server-relative path) and renders on detail pages, but v1 has **no file-upload endpoint** — that keeps the API surface small. Drop files into a served directory (or point the field at any reachable URL) and it just works; an upload endpoint can be added later without schema changes.
- **Category:** model `category` is a fixed enum (`heli, plane, car, drone, boat, other`) so filtering is simple.
- **Part type removed:** parts once had a free-text `part_type`, but with no controlled vocabulary it fragmented into inconsistent filter values and blurred into the name; it was dropped (migration `0004` removes the column and its stored values). Name + substring search covers the real use cases.
- **Part form fields:** the add/edit part form always shows the required name; every other field (quantity, cost, vendor, link/SKU, photo, notes) can be hidden from the form in `#/settings`. Hidden fields still exist on the record — edits just don't surface them — so nothing is ever wiped by hiding a field. The choice is stored server-side (`settings` table) and the part form reads it on load.
- **Cost & vendor:** `cost` is a nullable `REAL` (per-unit, in the user's configured currency; validated finite and ≥ 0) and `vendor` is a nullable free-text string. Costs render with `Intl.NumberFormat` using the `currency` setting (ISO-4217, default USD), falling back to a plain `$` formatting if the runtime doesn't support the code.
- **Link/SKU:** one string field `link` on parts. If it looks like an http(s) URL the UI renders it as a link, otherwise as a monospace SKU.
- **Update semantics:** `PUT` is a full replace of the record (the forms always submit everything), which avoids sparse-patch edge cases in a single-user tool. Quantity changes also have the atomic `POST .../quantity {delta}` endpoint, clamped at 0 server-side. The one deliberate exception is `POST /api/parts/bulk-edit`, which is a sparse tri-state update (per-field omitted/null/value) because a full-replace body makes no sense across a selection of different parts; the whole edit runs in a single transaction.
- **Association endpoints:** both sides of the M:N are manageable (`/models/:id/parts` and `/parts/:id/models`); the model side also supports full-set replace. Duplicate links are idempotent no-ops.
- **Usage log:** a "usage" is a part consumed on a model. Recording one decrements stock by the same amount (clamped at 0) so the drawer count and the log always agree; if you log more than is on hand the entry keeps the real quantity and the count clamps to 0. Entries are append-only: a mistaken entry is corrected by adjusting stock, not by deleting history. `model_id` is required on every entry because the whole point of the log is "which model did this go into?".
- **Sorting/searching:** case-insensitive substring search over the obvious text fields (name/manufacturer/notes for models, name/notes/link for parts); `LIKE` wildcards in user input are escaped.
- **Dates:** `date_acquired` is stored as an ISO-8601 date string, validated calendar-correctly (leap years included).
- **Static serving:** the backend serves the built SPA itself (hashed assets cached immutably, `index.html` no-cache) and returns JSON 404s for unknown `/api/*` paths. The SPA uses hash routing, so no server-side route fallback is needed.
- **Single process, no auth:** per the brief — one user, trusted network. SQLite pool is single-connection (WAL) which fits that profile and avoids `SQLITE_BUSY` entirely.
- **Postgres path:** everything below the HTTP handlers goes through `HangarRepo`/`ServiceApi` traits; a Postgres implementation would be a new module plus a feature flag, no route/service changes.
- **Catalog data lives in files, not migrations:** the parts catalog is imported from versioned, human-editable JSON files under `catalog-data/` (one file per model: `<manufacturer-slug>/<model-slug>.json`) into `catalog_manufacturers`/`catalog_models`/`catalog_parts`. Adding a model is "drop a file in the repo and restart" — no schema change, no code change, no manual admin step. The machine-readable format is `catalog-data/schema.json`; the Rust importer (`src/catalog.rs`) enforces the same rules with `file: field: message` errors and rejects unknown fields, so a typo in a hand-written file can't slip through.
- **JSON over CSV for catalog files:** the user floated CSV as an option; JSON won because the format is inherently nested and sparse (per-part diagram coordinates, optional part numbers/categories/notes) — CSV would need a wide table with mostly-empty columns and awkward escaping for multi-line notes, while JSON maps 1:1 onto the rows and validates structurally. One file per model keeps diffs tiny and reviews obvious.
- **Catalog import: idempotent upserts, never auto-delete:** re-imports match parts by `part_number` (exact) with a fallback to the unnumbered same-name row (so filling in a part number later re-keys the row instead of duplicating it); name-only parts match by case-insensitive name. Rows missing from a newer file version are **left in place** (with any inventory links intact) and logged as orphans for manual review/deletion via `DELETE /api/catalog/parts/:id` — this protects a user's inventory from a typo in a source file. The stored sha256 checksum short-circuits re-imports of unchanged files (they aren't even re-parsed). Invalid files are logged and skipped, never fatal to startup.
- **Import runs at startup (and via `import-catalog` CLI):** matches the single-user/self-hosted spirit — there is no "catalog admin UI" to keep in sync; the files in the repo are the source of truth and the DB is a materialized view of them. `cargo run -- import-catalog [file|dir]` covers ad-hoc re-imports and pre-commit validation of a new file (non-zero exit on any failure).
- **Placeholder example data:** `catalog-data/omp-hobby/m1.json` exists to prove the file → import → diagram pipeline end to end. Part *names* and diagram positions are plausible, but every `part_number` is deliberately `null` with a "placeholder — part number pending verification" note. **TODO (follow-up pass): research and populate the official OMP M1 part numbers in place.**
- **Catalog link & quantity semantics:** `models.catalog_model_id` is set only via `POST/DELETE /api/models/:id/link-catalog` (which validates category equality); `PUT /api/models/:id` deliberately does not touch it, so full-replace updates never wipe a catalog link. `owned_quantity` on `GET /api/catalog/models/:id` is `null` when no user model is linked (nothing to count against) and a live integer (0 when linked but not owned) otherwise; the optional `?model_id=` scopes it to one model. Add-to-inventory is idempotent per (catalog part, model): first call creates, later calls adjust the existing part's quantity (clamped at 0), so the same catalog part never yields duplicate inventory rows.
- **Generic diagrams per category:** real official diagrams are copyrighted and unobtainable, so the viewer renders generic hand-drawn SVG silhouettes from `frontend/src/lib/diagrams/` (`heli-generic.svg` is the real thing; other categories are simple placeholders) with the SVG text inlined at build time (`import.meta.glob(...?raw)`) — no extra requests, works in dev and prod. `diagram_asset` on a catalog model is a per-model override (e.g. a photo-based diagram later); when null/unknown the viewer falls back to `<category>-generic.svg`. Hotspot coordinates are percentages of the image (the SVGs use a 100×60 viewBox, so viewBox `(x, y)` → `diagram_x: x, diagram_y: y*100/60`).
- **Error contract:** deserialization failures (bad JSON, unknown enum values, bad path params) are mapped to the same structured 400 responses as domain validation, so clients can rely on one error shape.

## Project layout

```
├── Cargo.toml            # backend crate (bin `hangar` + lib for tests)
├── migrations/           # sqlx migrations (0001_init … 0007_catalog)
├── catalog-data/         # parts-catalog source files (imported at startup; see its README)
├── src/
│   ├── main.rs           # boot: env, pool, migrations, catalog import, serve (+ import-catalog CLI)
│   ├── lib.rs            # library root
│   ├── routes.rs         # axum router + handlers (thin)
│   ├── service.rs        # business rules behind ServiceApi trait
│   ├── repo/             # HangarRepo trait + SqliteRepo
│   ├── catalog.rs        # catalog file format, validation, checksum, import
│   ├── types.rs          # domain types, inputs, validation
│   ├── error.rs          # DomainError → JSON error responses
│   └── web.rs            # static SPA serving + API 404s
├── tests/api.rs          # end-to-end API tests (in-memory SQLite)
├── frontend/             # Svelte 5 + Vite + TS + Tailwind SPA
│   └── src/lib/diagrams/ # generic per-category SVG diagrams (heli, plane, car, drone, boat, other)
├── Dockerfile            # multi-stage: node build → rust build → slim runtime
└── docker-compose.yml    # single service + persistent hangar-data volume
```
