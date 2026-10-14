-- Sorting the depot (MsgWarehouse action 5, Character::sort_warehouse) moves
-- every item at once: one slot an item is checked only at the commit. The
-- positions stay 0..39; the old "out of the way first" step put them past it
-- and the CHECK refused every sort.
DO $$
DECLARE
    name text;
BEGIN
    FOR name IN
        SELECT conname FROM pg_constraint
        WHERE conrelid = 'warehouse_items'::regclass AND contype = 'u' AND NOT condeferrable
    LOOP
        EXECUTE format('ALTER TABLE warehouse_items DROP CONSTRAINT %I', name);
    END LOOP;
END $$;

ALTER TABLE warehouse_items
    ADD CONSTRAINT warehouse_items_slot UNIQUE (character_id, position) DEFERRABLE INITIALLY IMMEDIATE;
