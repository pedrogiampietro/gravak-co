-- Custom rarity is independent from the classic Conquer item quality encoded
-- in the final digit of item_type and from the plus/composition tier.
ALTER TABLE item_upgrades ADD COLUMN rarity INTEGER NOT NULL DEFAULT 0
    CHECK (rarity >= 0 AND rarity <= 4);
