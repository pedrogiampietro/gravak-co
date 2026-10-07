-- A +n Stone (730001 +1Stone .. 730009 +9Stone) is a +n item, which is what
-- composing reads; the ones made before this were +0 and composed nothing.
INSERT INTO item_upgrades (item_uid, plus)
SELECT item_uid, item_type - 730000 FROM inventory_items WHERE item_type BETWEEN 730001 AND 730009
ON CONFLICT (item_uid) DO UPDATE SET plus = excluded.plus WHERE item_upgrades.plus = 0;

INSERT INTO item_upgrades (item_uid, plus)
SELECT item_uid, item_type - 730000 FROM warehouse_items WHERE item_type BETWEEN 730001 AND 730009
ON CONFLICT (item_uid) DO UPDATE SET plus = excluded.plus WHERE item_upgrades.plus = 0;

INSERT INTO item_upgrades (item_uid, plus)
SELECT item_uid, item_type - 730000 FROM guild_warehouse_items WHERE item_type BETWEEN 730001 AND 730009
ON CONFLICT (item_uid) DO UPDATE SET plus = excluded.plus WHERE item_upgrades.plus = 0;
