-- New characters start with the level 1 weapons of their class, plain (no
-- Super): migration 94 gave Super weapons of levels 5 to 8 (an Archer got a
-- Super BambooBow). Trojans and Warriors take the LuckyBlade, Archers the
-- LuckyBow (with the LuckyArrows), Taoists the LuckyBacksword.
UPDATE starter_items SET item_type = 410301 WHERE starter_id = 1; -- Trojan: LuckyBlade
UPDATE starter_items SET item_type = 410301 WHERE starter_id = 2; -- Warrior: LuckyBlade
UPDATE starter_items SET item_type = 500301 WHERE starter_id = 3; -- Archer: LuckyBow
UPDATE starter_items SET item_type = 421301 WHERE starter_id = 5; -- Taoist: LuckyBacksword
