CREATE TABLE IF NOT EXISTS character_skills (
    character_id BIGINT NOT NULL CONSTRAINT fk_skill_character REFERENCES characters(character_id) ON DELETE CASCADE,
    skill_id BIGINT NOT NULL CHECK (skill_id > 0),
    level BIGINT NOT NULL DEFAULT 1 CHECK (level > 0),
    experience BIGINT NOT NULL DEFAULT 0 CHECK (experience >= 0),
    proficiency BIGINT NOT NULL DEFAULT 0 CHECK (proficiency >= 0),
    PRIMARY KEY (character_id, skill_id)
);
