-- Hangar v1 schema
CREATE TABLE models (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    name          TEXT NOT NULL,
    category      TEXT NOT NULL CHECK (category IN ('heli', 'plane', 'car', 'drone', 'boat', 'other')),
    manufacturer  TEXT,
    notes         TEXT,
    date_acquired TEXT,
    status        TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'retired', 'sold')),
    photo_url     TEXT,
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE TABLE parts (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL,
    part_type  TEXT,
    quantity   INTEGER NOT NULL DEFAULT 0 CHECK (quantity >= 0),
    notes      TEXT,
    link       TEXT,
    photo_url  TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE TABLE model_parts (
    model_id   INTEGER NOT NULL REFERENCES models(id) ON DELETE CASCADE,
    part_id    INTEGER NOT NULL REFERENCES parts(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (model_id, part_id)
);

CREATE INDEX idx_models_category ON models (category);
CREATE INDEX idx_models_name ON models (name);
CREATE INDEX idx_parts_name ON parts (name);
CREATE INDEX idx_parts_type ON parts (part_type);
CREATE INDEX idx_model_parts_part ON model_parts (part_id);
