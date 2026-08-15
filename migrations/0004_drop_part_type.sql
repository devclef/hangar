-- part_type is removed: an uncontrolled free-text "type" produced a
-- fragmented filter vocabulary, and name + search covered the real cases.
-- NOTE: drops any stored type values (SQLite >= 3.35 required for DROP COLUMN).
DROP INDEX idx_parts_type;
ALTER TABLE parts DROP COLUMN part_type;
