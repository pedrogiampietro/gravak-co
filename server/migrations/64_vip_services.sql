-- VIP services: which drops go straight to a VIP's bag (a bit per kind:
-- 1 silver, 2 Meteors and DragonBalls, 4 gems, 8 potions, 16 Unique or
-- better equipment, 32 any equipment). Silver and Meteors/DragonBalls to
-- start, as before.
ALTER TABLE characters ADD COLUMN IF NOT EXISTS loot_filter BIGINT NOT NULL DEFAULT 3;
