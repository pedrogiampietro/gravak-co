-- Player shops (booths): the name over a seller's booth, and what they list,
-- kept so a shop opened again (after a disconnect too) has its name and
-- listings back. A listing whose item left the bag is dropped on reopening.
CREATE TABLE IF NOT EXISTS player_shops (
    character_id BIGINT PRIMARY KEY REFERENCES characters(character_id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 24)
);

CREATE TABLE IF NOT EXISTS player_shop_listings (
    item_uid BIGINT PRIMARY KEY,
    character_id BIGINT NOT NULL REFERENCES characters(character_id) ON DELETE CASCADE,
    price BIGINT NOT NULL CHECK (price > 0)
);
