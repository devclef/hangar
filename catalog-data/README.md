# Parts catalog data

Versioned, human-editable source files for the **reference parts catalog**.
Each file describes one known manufacturer/model combination and the official
parts for it. The files are imported into the `catalog_manufacturers` /
`catalog_models` / `catalog_parts` tables automatically **on startup** and on
demand via the CLI — you never edit the database directly, and adding a model
is "drop a file in and restart" (no code change, no schema change).

## Layout & naming

```
catalog-data/
├── README.md              ← this file
├── schema.json            ← JSON Schema for the file format (mirrored by the Rust validator)
└── <manufacturer-slug>/
    └── <model-slug>.json  ← one file per model
```

- Slugs are lowercase `kebab-case` (letters, digits, `-`), e.g.
  `omp-hobby/m1.json`, `sfc-bushings/trex-450.json`.
- Files named `schema.json` are format documents and are **ignored** by the
  scanner (so the JSON Schema can live right here).
- The directory/file slugs are only for tidy storage: the **identity** of a
  catalog entry is the `(manufacturer, model)` pair *inside* the file (plus
  the file path + sha256 recorded as provenance), so renaming a file does not
  duplicate anything — the upsert matches on the content.
- The machine-readable format spec is `schema.json`. The Rust importer
  (`src/catalog.rs`) enforces exactly the same rules and reports errors as
  `<file>: <field>: <message>` (unknown fields are rejected — a typo in a
  hand-written file will not slip through silently).

## File format

```json
{
  "manufacturer": "OMP Hobby",
  "model": "M1",
  "category": "heli",
  "diagram_asset": "heli-generic.svg",
  "parts": [
    {
      "name": "Main blade grip set",
      "part_number": "OSHM1013",
      "category": "Blade grip",
      "notes": "Includes bearings",
      "diagram_x": 62.5,
      "diagram_y": 18.0
    },
    {
      "name": "Tail blade grip set",
      "part_number": null,
      "category": "Blade grip",
      "notes": "Part number not yet verified",
      "diagram_x": 91.0,
      "diagram_y": 47.0
    }
  ]
}
```

| Field            | Required | Notes |
| ---------------- | -------- | ----- |
| `manufacturer`   | yes      | Display name (e.g. "OMP Hobby"). Matching is exact after trimming — keep the casing stable across files. |
| `model`          | yes      | Display name (e.g. "M1"). |
| `category`       | yes      | One of `heli`, `plane`, `car`, `drone`, `boat`, `other`. User models can only be linked to catalog models with the same category. |
| `diagram_asset`  | no       | Per-model diagram override: a plain file name in `frontend/src/lib/diagrams/` (no `/`, no `..`). Omit for the generic per-category SVG (`<category>-generic.svg`). |
| `parts`          | yes      | Array (may be empty). Duplicates (same `part_number`, or same name when the number is absent) are rejected. |
| `parts[].name`   | yes      | Part name. |
| `parts[].part_number` | no | Official manufacturer part number. **Nullable on purpose** — you can add a row before the number is verified. Empty string = null. This is the stable identity of the row on re-import (exact match); without one, the case-insensitive name is. |
| `parts[].category` | no    | Free-text grouping for the legend (e.g. "Blade grip", "Tail boom"). Not the old user-part `part_type`. |
| `parts[].notes`  | no       | Free text (fitment caveats, what's included, ...). |
| `parts[].diagram_x` / `diagram_y` | no | Hotspot position, **percentages 0–100** of the diagram width/height. Give both or neither; `null` = not diagram-placeable (e.g. a hardware bag). |

## Diagram coordinates

`diagram_x`/`diagram_y` are percentages of the diagram image itself, so they
scale with the rendered size. The generic SVGs in
`frontend/src/lib/diagrams/` all use a `100 × 60` viewBox, which makes
placement easy: a point at viewBox coordinate `(vx, vy)` is
`diagram_x = vx`, `diagram_y = vy * 100 / 60`. Example: the rotor hub at
viewBox `(37, 10.8)` → `diagram_x: 37.0, diagram_y: 18.0`.

To pick coordinates visually, open the SVG in a browser and hover — or place
a test pin and adjust until it sits on the part.

## Adding a new model (or a new manufacturer)

1. Create the file: `catalog-data/<manufacturer-slug>/<model-slug>.json`
   (create the manufacturer directory for a new manufacturer).
2. Validate it:
   ```bash
   cargo run -- import-catalog catalog-data/<manufacturer-slug>/<model-slug>.json
   ```
   The command prints the result (created/updated parts, orphans) and exits
   non-zero with a `file: field: message` line on any validation error.
   `schema.json` also validates it in editors that support JSON Schema.
3. Restart the app (or leave it — the next startup picks the file up). The
   startup log shows a summary like
   `catalog import finished files=1 created=1 updated=0 unchanged=0 failed=0`.

## Re-import behavior (read this before editing files)

- Re-importing is an **upsert**, keyed by `(manufacturer, model)`. Part
  matching on re-import:
  - a file part **with** a `part_number` matches an existing row with the
    same number; failing that, it matches an existing row **without** a
    number and with the same (case-insensitive) name — so a follow-up edit
    that *fills in* a part number re-keys the existing row instead of
    orphaning it;
  - a file part **without** a `part_number` matches an unnumbered existing
    row by (case-insensitive) name only (it never strips a number).
  
  Edits update the existing rows; nothing is ever duplicated.
- The stored sha256 of the file short-circuits re-imports: an unchanged file
  is not even re-parsed at startup.
- **Parts you remove from a file are NOT deleted.** The row is left in place
  (with any inventory links intact) and the importer logs a warning:
  `catalog part "Tail boom" (id=42) no longer present in
  catalog-data/omp-hobby/m1.json — left in place, review manually`. Delete
  orphans explicitly with `DELETE /api/catalog/parts/:id` (inventory parts
  survive; their trace link becomes `null`). This protects a user's inventory
  from a typo in a source file.
- Renaming `manufacturer` or `model` in a file creates a **new** catalog
  entry and orphans the old one — do it deliberately, then clean up the
  orphan via the admin delete.
- Invalid files are logged and skipped; they never block startup or crash the
  app.

## Current contents

- `omp-hobby/m1.json` — OMP Hobby M1 (heli). **Placeholder data**: part names
  and diagram positions are real/sensible, but every `part_number` is `null`
  ("part number pending verification"). A follow-up pass is required to
  research and fill in the official OMP part numbers. See the main README.
