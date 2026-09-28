-- Item artifacts are kept separately from temporary refinery souls.
CREATE TABLE IF NOT EXISTS item_artifacts (
    item_uid INTEGER PRIMARY KEY,
    artifact_type INTEGER NOT NULL,
    artifact_level INTEGER NOT NULL DEFAULT 0,
    effect INTEGER NOT NULL DEFAULT 0,
    expires_at INTEGER NOT NULL
);
