-- Reference parts catalog: data for known manufacturer/model combinations,
-- imported from version-controlled files under catalog-data/ (see
-- src/catalog.rs for the file format and the import/upsert rules).
--
-- These tables are populated exclusively by the importer (at startup and
-- via `hangar import-catalog`); there are no create/update HTTP endpoints
-- for them. Inventory traces back to the catalog through the two nullable
-- FK columns added at the bottom.
CREATE TABLE catalog_manufacturers (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL UNIQUE,
    notes      TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE TABLE catalog_models (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    manufacturer_id INTEGER NOT NULL REFERENCES catalog_manufacturers(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    category        TEXT NOT NULL CHECK (category IN ('heli', 'plane', 'car', 'drone', 'boat', 'other')),
    -- Per-model diagram override (a file in frontend/src/lib/diagrams/);
    -- NULL means the frontend falls back to the generic per-category SVG.
    diagram_asset   TEXT,
    -- Provenance: which catalog file this row came from, and the sha256 of
    -- its contents. Re-imports short-circuit when both still match.
    source_file     TEXT NOT NULL,
    source_checksum TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    UNIQUE (manufacturer_id, name)
);

CREATE TABLE catalog_parts (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    catalog_model_id INTEGER NOT NULL REFERENCES catalog_models(id) ON DELETE CASCADE,
    name             TEXT NOT NULL,
    part_number      TEXT,
    -- Free-text grouping/legend (e.g. "Blade grip", "Tail boom"); not the
    -- old user-part `part_type`.
    category         TEXT,
    notes            TEXT,
    -- Hotspot position on the diagram, as percentages 0-100 of the image
    -- width/height. NULL = not diagram-placeable (e.g. a hardware bag).
    diagram_x        REAL,
    diagram_y        REAL,
    created_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX idx_catalog_models_manufacturer ON catalog_models (manufacturer_id);
CREATE INDEX idx_catalog_parts_model ON catalog_parts (catalog_model_id);
CREATE INDEX idx_catalog_parts_number ON catalog_parts (catalog_model_id, part_number);

-- Optional trace links from user data back to the catalog. Both are
-- nullable on purpose: hand-created models/parts keep NULL, and deleting a
-- catalog row (or unlinking a model) never destroys inventory data.
ALTER TABLE parts ADD COLUMN catalog_part_id INTEGER REFERENCES catalog_parts(id) ON DELETE SET NULL;
CREATE INDEX idx_parts_catalog_part ON parts (catalog_part_id);

ALTER TABLE models ADD COLUMN catalog_model_id INTEGER REFERENCES catalog_models(id) ON DELETE SET NULL;
CREATE INDEX idx_models_catalog_model ON models (catalog_model_id);
