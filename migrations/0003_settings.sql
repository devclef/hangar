-- Simple key/value store for user preferences. The whole settings document
-- is kept as one JSON string under the key 'app'.
CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
