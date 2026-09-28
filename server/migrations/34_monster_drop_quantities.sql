ALTER TABLE monster_drops ADD COLUMN min_quantity INTEGER NOT NULL DEFAULT 1 CHECK (min_quantity >= 1);
ALTER TABLE monster_drops ADD COLUMN max_quantity INTEGER NOT NULL DEFAULT 1 CHECK (max_quantity >= 1);

UPDATE monster_drops
SET min_quantity = 1,
    max_quantity = 1
WHERE max_quantity < min_quantity;
