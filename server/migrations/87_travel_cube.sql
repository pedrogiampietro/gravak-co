-- The Travel Cube (special_items.rs): the Store sells it.
INSERT INTO store_products (product_id, category, name, description, item_type, amount, vip_days, price_cps, daily_stock, sort)
VALUES
    (26, 3, 'Travel Cube', 'A black cube that carries you to any city, once every 10 minutes. It stays with you on death; only a trade passes it on.', 723990, 1, 0, 1200, NULL, 26)
ON CONFLICT (product_id) DO NOTHING;
