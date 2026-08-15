-- Cost (per unit, in the user's configured currency) and vendor for parts.
ALTER TABLE parts ADD COLUMN cost REAL;
ALTER TABLE parts ADD COLUMN vendor TEXT;
