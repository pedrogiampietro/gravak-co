CREATE TABLE IF NOT EXISTS warehouse_balances (
    character_id INTEGER PRIMARY KEY REFERENCES characters(character_id) ON DELETE CASCADE,
    silver INTEGER NOT NULL DEFAULT 0 CHECK (silver >= 0 AND silver <= 4294967295)
);
