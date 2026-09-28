-- Temporary refinery effects are kept by item UID so moving an item between
-- inventory/equipment does not lose its soul/refinery state.
CREATE TABLE IF NOT EXISTS item_refineries (
    item_uid INTEGER PRIMARY KEY,
    refinery_type INTEGER NOT NULL,
    refinery_level INTEGER NOT NULL DEFAULT 0,
    effect INTEGER NOT NULL DEFAULT 0,
    expires_at INTEGER NOT NULL
);
