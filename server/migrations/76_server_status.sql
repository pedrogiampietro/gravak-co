-- What the website shows of the game server: who is online now, the most
-- ever seen at once, and when the server last wrote (a stale row means the
-- server is down). One row, written every 30 seconds.
CREATE TABLE IF NOT EXISTS server_status (
    id SMALLINT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    online INTEGER NOT NULL DEFAULT 0 CHECK (online >= 0),
    record INTEGER NOT NULL DEFAULT 0 CHECK (record >= 0),
    updated_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM now())::BIGINT)
);
