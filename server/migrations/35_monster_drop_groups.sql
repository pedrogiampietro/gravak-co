ALTER TABLE monster_drops ADD COLUMN drop_group BIGINT NOT NULL DEFAULT 0 CHECK (drop_group >= 0);
ALTER TABLE monster_drops ADD COLUMN min_monster_level BIGINT NOT NULL DEFAULT 0 CHECK (min_monster_level >= 0);
ALTER TABLE monster_drops ADD COLUMN max_monster_level BIGINT NOT NULL DEFAULT 255 CHECK (max_monster_level >= 0);

UPDATE monster_drops
SET max_monster_level = 255
WHERE max_monster_level < min_monster_level;
