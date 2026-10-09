-- The Rifts became crypts (narrow corridors laid cell by cell, with our own
-- art): every crypt's entrance room has its middle at cell 13, 13.
UPDATE maps
   SET revive_point_x = 13, revive_point_y = 13
 WHERE id IN (9101, 9102, 9103, 9111, 9112, 9113, 9121, 9122, 9123);
