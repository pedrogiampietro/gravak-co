-- The browser client's settings files (client/crates/sandbox/src/saved.rs),
-- kept with the account so they follow the player to any browser: the
-- Options, HUD, shortcuts, helper and Sell All files, by name. Written by
-- the site (web/src/app/api/settings).
CREATE TABLE IF NOT EXISTS account_settings (
    account_id BIGINT NOT NULL REFERENCES accounts(account_id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (name ~ '^[a-z0-9_]{1,40}\.ini$'),
    body TEXT NOT NULL CHECK (length(body) <= 65536),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (account_id, name)
);
