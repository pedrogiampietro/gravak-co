-- Optional item progression state is kept separate from the three item
-- containers because an item can move between inventory and equipment while
-- retaining its UID. This also leaves the legacy item table layout intact.
CREATE TABLE IF NOT EXISTS item_upgrades (
    item_uid INTEGER PRIMARY KEY,
    plus INTEGER NOT NULL DEFAULT 0 CHECK (plus >= 0 AND plus <= 12),
    gem_one INTEGER NOT NULL DEFAULT 0,
    gem_two INTEGER NOT NULL DEFAULT 0,
    socket_one INTEGER NOT NULL DEFAULT 0,
    socket_two INTEGER NOT NULL DEFAULT 0,
    bless INTEGER NOT NULL DEFAULT 0,
    enchant INTEGER NOT NULL DEFAULT 0
);
