-- The second rebirth asks an ExemptionToken (Alex, in the Market); the
-- Store sells it.
INSERT INTO store_products (product_id, category, name, description, item_type, amount, vip_days, price_cps, daily_stock, sort)
VALUES
    (25, 2, 'Exemption Token', 'Alex, in the Market, grants a second rebirth for one: at the top rank of your class and level 120 (110 for Water Taoists).', 723701, 1, 0, 1500, NULL, 25)
ON CONFLICT (product_id) DO NOTHING;
