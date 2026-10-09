-- The Greater and Abyssal Rifts take creatures of their own: new models made
-- in Meshy and brought in by the content build (content/monsters.toml,
-- [[creature]]). The Greater Rift keeps its painted bat (962) until the flesh
-- golem comes.
UPDATE monster_types SET look = 982, name = 'GuerreiroOsso'   WHERE id = 9160;
UPDATE monster_types SET look = 981, name = 'SacerdoteSangue' WHERE id = 9162;
UPDATE monster_types SET look = 984, name = 'ColossoSangue'   WHERE id = 9169;
UPDATE monster_types SET look = 985                           WHERE id = 9170;
UPDATE monster_types SET look = 986, name = 'Espreitador'     WHERE id = 9171;
UPDATE monster_types SET look = 987                           WHERE id = 9172;
UPDATE monster_types SET look = 988, name = 'TitaAbismo'      WHERE id = 9179;
