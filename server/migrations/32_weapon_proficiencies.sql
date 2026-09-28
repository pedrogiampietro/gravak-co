CREATE TABLE IF NOT EXISTS character_weapon_proficiencies (
    character_id INTEGER NOT NULL,
    weapon_family INTEGER NOT NULL,
    level INTEGER NOT NULL DEFAULT 1,
    experience INTEGER NOT NULL DEFAULT 0,
    proficiency INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (character_id, weapon_family),
    FOREIGN KEY (character_id) REFERENCES characters(character_id) ON DELETE CASCADE
);
