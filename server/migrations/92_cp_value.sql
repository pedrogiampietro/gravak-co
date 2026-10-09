-- A CP is now worth R$ 0,10 (R$ 10 buy 100 CPs) instead of R$ 0,50: every
-- amount of CPs is five times what it was, so nothing changes in value.
-- The packages, the Store's prices, the CPs characters hold or owe, booths
-- priced in CPs and the orders and purchases already made.
UPDATE cp_packages SET cps = cps * 5;

UPDATE store_products SET price_cps = price_cps * 5;
UPDATE store_products
   SET price_steps = (
       SELECT string_agg((step::BIGINT * 5)::TEXT, ',' ORDER BY position)
         FROM unnest(string_to_array(price_steps, ',')) WITH ORDINALITY AS steps (step, position)
   )
 WHERE price_steps IS NOT NULL AND price_steps <> '';
UPDATE store_purchases SET price_cps = price_cps * 5;

UPDATE characters SET cps = cps * 5, cps_debt = cps_debt * 5 WHERE cps > 0 OR cps_debt > 0;

UPDATE player_shop_listings SET price = price * 5 WHERE cps;

UPDATE payment_orders SET cps = cps * 5, reversed_cps = reversed_cps * 5;
