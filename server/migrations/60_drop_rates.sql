-- The Pheasant's Meteor (80%) and DragonBall (9%) were the fixture that
-- proved the drop table works; every monster now has Redux's own chances
-- for both (0.72% and 0.0556%, in the server), so the fixture goes.
DELETE FROM monster_drops WHERE monster_type_id = 1 AND item_id IN (1088000, 1088001);
