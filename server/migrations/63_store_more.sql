-- More for the Store: stronger experience potions (the client's own ExpPill
-- and ExpAmrita), the Heaven's Blessings in a category of their own,
-- garments under Cosmetics, and how strong a character's experience boost is.

ALTER TABLE characters ADD COLUMN IF NOT EXISTS experience_multiplier BIGINT NOT NULL DEFAULT 2;

-- 0 VIP, 1 Experience, 2 Upgrades, 3 Blessings, 4 Cosmetics.
UPDATE store_products SET category = 3 WHERE product_id IN (12, 13, 14);

INSERT INTO store_products (product_id, category, name, description, item_type, amount, vip_days, price_cps, daily_stock, sort)
VALUES
    (15, 1, 'Exp Pill (3x)', 'Triples the experience you gain for two hours.', 720393, 1, 0, 180, NULL, 15),
    (16, 1, 'Exp Amrita (5x)', 'Five times the experience you gain for two hours.', 720394, 1, 0, 450, 20, 16),
    (30, 4, 'Dark Wizard', 'A garment worn over your armor.', 181355, 1, 0, 600, NULL, 30),
    (31, 4, 'Royal Dignity', 'Official dress of the royals, worn over your armor.', 181385, 1, 0, 600, NULL, 31),
    (32, 4, 'White Phoenix', 'A garment worn over your armor.', 181305, 1, 0, 500, NULL, 32),
    (33, 4, 'Angelical Dress', 'A dress for sweet girls and lovely boys, worn over your armor.', 182325, 1, 0, 800, NULL, 33),
    (34, 4, 'Flying Feather', 'Made of feathers: slim and graceful. For ladies only.', 182355, 1, 0, 800, NULL, 34)
ON CONFLICT DO NOTHING;
