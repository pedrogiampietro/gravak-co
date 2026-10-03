CREATE TABLE IF NOT EXISTS monster_drops (
  monster_type_id BIGINT NOT NULL CONSTRAINT fk_monster_drop_type REFERENCES monster_types(id) ON DELETE CASCADE,
  item_id BIGINT NOT NULL,
  chance_percent BIGINT NOT NULL DEFAULT 100 CHECK (chance_percent BETWEEN 0 AND 100),
  PRIMARY KEY (monster_type_id, item_id)
);

INSERT INTO monster_drops (monster_type_id, item_id, chance_percent)
VALUES
  (1, 1088001, 80),
  (1, 1088000, 9)
ON CONFLICT DO NOTHING;

SELECT sync_identity_sequences();
