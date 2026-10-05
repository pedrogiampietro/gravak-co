CREATE TABLE IF NOT EXISTS character_weapon_proficiencies (
    character_id BIGINT NOT NULL,
    weapon_family BIGINT NOT NULL,
    level BIGINT NOT NULL DEFAULT 1,
    experience BIGINT NOT NULL DEFAULT 0,
    proficiency BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (character_id, weapon_family),
    FOREIGN KEY (character_id) REFERENCES characters(character_id) ON DELETE CASCADE
);
