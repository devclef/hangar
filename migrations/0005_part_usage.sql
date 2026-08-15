-- Log of parts consumed against a model (repairs, builds, swaps).
-- Recording a usage also decrements the part's stock (clamped at 0),
-- done in one transaction in the repo layer.
CREATE TABLE part_usage (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    part_id    INTEGER NOT NULL REFERENCES parts(id) ON DELETE CASCADE,
    model_id   INTEGER NOT NULL REFERENCES models(id) ON DELETE CASCADE,
    quantity   INTEGER NOT NULL CHECK (quantity > 0),
    notes      TEXT,
    used_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX idx_part_usage_part ON part_usage (part_id);
CREATE INDEX idx_part_usage_model ON part_usage (model_id);
CREATE INDEX idx_part_usage_used_at ON part_usage (used_at);
