-- The level a character had at its first rebirth: the second rebirth adds
-- that rebirth's bonus points again to its own (0: not reborn yet, or reborn
-- before this migration, when the lowest level a rebirth asks is assumed).
ALTER TABLE characters ADD COLUMN IF NOT EXISTS first_rebirth_level INTEGER NOT NULL DEFAULT 0;
