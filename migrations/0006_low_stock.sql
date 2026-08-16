-- Per-part opt-out of the "low" quantity badge. Users commonly keep a
-- single spare of a part and do not want it flagged as low.
ALTER TABLE parts ADD COLUMN low_stock_enabled INTEGER NOT NULL DEFAULT 1;
