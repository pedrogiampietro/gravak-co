-- The Store's prices back where they were before migration 92, in CPs (7
-- days of VIP 90, 30 days 290): CPs cost less now (R$ 10 buy 100), what they
-- buy does not. The packages and the CPs characters hold stay as 92 left them.
UPDATE store_products SET price_cps = price_cps / 5;
UPDATE store_products
   SET price_steps = (
       SELECT string_agg((step::BIGINT / 5)::TEXT, ',' ORDER BY position)
         FROM unnest(string_to_array(price_steps, ',')) WITH ORDINALITY AS steps (step, position)
   )
 WHERE price_steps IS NOT NULL AND price_steps <> '';
UPDATE store_purchases SET price_cps = price_cps / 5;

-- The CelestialStone the first rebirth asks (the ExemptionToken, for the
-- second, is product 25).
INSERT INTO store_products (product_id, category, name, description, item_type, amount, vip_days, price_cps, daily_stock, sort)
VALUES
    (24, 2, 'Celestial Stone', 'Alex, in the Market, grants a first rebirth for one: at the top rank of your class and level 120 (110 for Water Taoists).', 721259, 1, 0, 200, NULL, 24)
ON CONFLICT (product_id) DO NOTHING;
