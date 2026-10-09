-- The Rifts' creatures see as far as a room and the wall beside it (6
-- cells), not across the crypt: from 10 cells they all came after the first
-- hero they saw and piled up against the walls between.
UPDATE monster_types SET view_range = 6 WHERE id IN (9150, 9151, 9152, 9160, 9161, 9162, 9170, 9171, 9172);
