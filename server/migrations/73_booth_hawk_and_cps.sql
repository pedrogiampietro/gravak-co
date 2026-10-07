-- Booths as the original: the line a seller hawks (DialogHawk, shown in a
-- bubble over the booth) holds up to 60 letters, and an item may be priced
-- in CPs (MsgItem 29, `BoothAddCPs`) instead of silver.
ALTER TABLE player_shops DROP CONSTRAINT IF EXISTS player_shops_name_check;
ALTER TABLE player_shops ADD CONSTRAINT player_shops_name_check CHECK (length(name) BETWEEN 1 AND 60);

ALTER TABLE player_shop_listings ADD COLUMN IF NOT EXISTS cps BOOLEAN NOT NULL DEFAULT FALSE;
