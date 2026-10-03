-- Field bosses. The reference servers carry these monster types but do not
-- place them: each now guards the strongest hunting spot of a city map,
-- returns two hours after it falls and drops a Meteor, often a DragonBall,
-- sometimes a MeteorScroll and a normal gem. All of this is our choice.
UPDATE monster_types SET respawn_seconds = 7200 WHERE id IN (3644, 3645, 3646, 4032, 4033, 4034, 4035, 4036);

INSERT INTO monster_spawns (monster_type_id, map_id, x, y, amount, radius)
VALUES
  (3644, 1001, 352, 290, 1, 0),
  (3645, 1001, 344, 263, 1, 0),
  (3646, 1001, 360, 280, 1, 0),
  (4032, 1011, 589, 808, 1, 0),
  (4035, 1015, 500, 505, 1, 0),
  (4034, 1020, 313, 600, 1, 0),
  (4036, 1000, 207, 374, 1, 0),
  (4033, 1000, 252, 310, 1, 0);

INSERT INTO monster_drops (monster_type_id, item_id, chance_percent)
VALUES
  (3644, 1088001, 100),
  (3644, 1088000, 50),
  (3644, 720027, 20),
  (3644, 700001, 10),
  (3645, 1088001, 100),
  (3645, 1088000, 50),
  (3645, 720027, 20),
  (3645, 700001, 10),
  (3646, 1088001, 100),
  (3646, 1088000, 50),
  (3646, 720027, 20),
  (3646, 700001, 10),
  (4032, 1088001, 100),
  (4032, 1088000, 50),
  (4032, 720027, 20),
  (4032, 700001, 10),
  (4033, 1088001, 100),
  (4033, 1088000, 50),
  (4033, 720027, 20),
  (4033, 700001, 10),
  (4034, 1088001, 100),
  (4034, 1088000, 50),
  (4034, 720027, 20),
  (4034, 700001, 10),
  (4035, 1088001, 100),
  (4035, 1088000, 50),
  (4035, 720027, 20),
  (4035, 700001, 10),
  (4036, 1088001, 100),
  (4036, 1088000, 50),
  (4036, 720027, 20),
  (4036, 700001, 10)
ON CONFLICT DO NOTHING;

SELECT sync_identity_sequences();
