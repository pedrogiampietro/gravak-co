-- The Rifts' labyrinths (docs/fendas-design.md): maps of ours that the
-- content build generates from content/rifts.toml (client/crates/content/src/
-- rifts.rs) into content/build/client/map/map/gravak-rift-<id>.DMap, where
-- the server reads them through GRAVAK_CONTENT.
--
-- The revive point is the middle of the entrance room (room 0, 0). Flags:
-- RECORD_DISABLED (4: who logs out here comes back in Twin City),
-- PK_DISABLED (8), TELEPORT_DISABLED (64: no scrolls in or out) and
-- FLY_DISABLED (512: no flying over the pits that make the walls).
INSERT INTO maps (id, map_id, path, revive_point_x, revive_point_y, flags, weather, reborn_map, color)
VALUES
    (9101, 9101, 'gravak-rift-9101.cmap', 64, 225, 588, 0, 1002, 4294967295),
    (9102, 9102, 'gravak-rift-9102.cmap', 64, 225, 588, 0, 1002, 4294967295),
    (9103, 9103, 'gravak-rift-9103.cmap', 64, 225, 588, 0, 1002, 4294967295),
    (9111, 9111, 'gravak-rift-9111.cmap', 64, 257, 588, 0, 1002, 4294967295),
    (9112, 9112, 'gravak-rift-9112.cmap', 63, 258, 588, 0, 1002, 4294967295),
    (9113, 9113, 'gravak-rift-9113.cmap', 64, 257, 588, 0, 1002, 4294967295),
    (9121, 9121, 'gravak-rift-9121.cmap', 63, 290, 588, 0, 1002, 4294967295),
    (9122, 9122, 'gravak-rift-9122.cmap', 63, 290, 588, 0, 1002, 4294967295),
    (9123, 9123, 'gravak-rift-9123.cmap', 63, 290, 588, 0, 1002, 4294967295)
ON CONFLICT (id) DO NOTHING;
