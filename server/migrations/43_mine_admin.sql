-- Twin City's Mine Admin (Redux [4293]), who sends players to the fourth mine.
INSERT INTO npcs (id, name, kind, look, map_id, x, y, base, sort, level, life, defense, magic_defense)
VALUES (4293, 'MineAdmin', 2, 1176, 1002, 45, 395, 0, 0, 0, 0, 0, 0)
ON CONFLICT DO NOTHING;

SELECT sync_identity_sequences();
