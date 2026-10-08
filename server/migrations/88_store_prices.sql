-- The Store's prices, about 1/6 of before (1 CP is worth about 9 cents: 25
-- CPs cost $2, 100 CPs $9), 15 days of VIP between the week and the month,
-- and a price that climbs with each purchase of the day (price_steps, the
-- prices of the first, second... purchase; past the last each costs 1.8
-- times the one before).
ALTER TABLE store_products ADD COLUMN IF NOT EXISTS price_steps TEXT;

INSERT INTO store_products (product_id, category, name, description, item_type, amount, vip_days, price_cps, daily_stock, sort)
VALUES
    (3, 0, '15 Days of VIP', 'VIP for your whole account: +10% experience from monsters, silver, Meteors and DragonBalls from your kills go straight to your bag, and the Conductresses carry you for free.', 0, 1, 15, 160, NULL, 2)
ON CONFLICT (product_id) DO NOTHING;

UPDATE store_products SET price_cps = 90, sort = 1 WHERE product_id = 1;   -- 7 days of VIP
UPDATE store_products SET price_cps = 290, sort = 3 WHERE product_id = 2;  -- 30 days of VIP
-- The Exp Potion (double experience for an hour): 30, 45, 90, 160, 290...
UPDATE store_products SET price_cps = 30, price_steps = '30,45,90,160,290' WHERE product_id = 10;
UPDATE store_products SET price_cps = 60 WHERE product_id = 11;    -- Exp Ball
UPDATE store_products SET price_cps = 90 WHERE product_id = 15;    -- Exp Pill (3x)
UPDATE store_products SET price_cps = 160 WHERE product_id = 16;   -- Exp Amrita (5x)
UPDATE store_products SET price_cps = 50 WHERE product_id = 12;    -- Heaven's Blessing, 3 days
UPDATE store_products SET price_cps = 90 WHERE product_id = 13;    -- 7 days
UPDATE store_products SET price_cps = 290 WHERE product_id = 14;   -- 30 days
UPDATE store_products SET price_cps = 2 WHERE product_id = 20;     -- Meteor
UPDATE store_products SET price_cps = 20 WHERE product_id = 21;    -- Meteor Scroll
UPDATE store_products SET price_cps = 35 WHERE product_id = 22;    -- DragonBall
UPDATE store_products SET price_cps = 340 WHERE product_id = 23;   -- DragonBall Scroll
UPDATE store_products SET price_cps = 250 WHERE product_id = 25;   -- Exemption Token
UPDATE store_products SET price_cps = 300 WHERE product_id = 26;   -- Travel Cube
UPDATE store_products SET price_cps = 100 WHERE product_id IN (30, 31);  -- Dark Wizard, Royal Dignity
UPDATE store_products SET price_cps = 80 WHERE product_id = 32;          -- White Phoenix
UPDATE store_products SET price_cps = 130 WHERE product_id IN (33, 34);  -- Angelical Dress, Flying Feather
