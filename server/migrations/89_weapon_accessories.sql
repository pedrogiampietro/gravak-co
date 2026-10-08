-- Weapon accessories (content/accessories): the right accessory (15) dresses
-- the right hand's weapon of its kind in its look, the left (16) the left
-- hand's.
ALTER TABLE equipment_items DROP CONSTRAINT IF EXISTS equipment_items_position_check;
ALTER TABLE equipment_items ADD CONSTRAINT equipment_items_position_check
    CHECK ((position >= 1 AND position <= 9) OR position IN (15, 16));
