-- Married couples: one row per spouse (database character ids).
CREATE TABLE IF NOT EXISTS marriages (
  character_id INTEGER PRIMARY KEY,
  spouse_id INTEGER NOT NULL,
  spouse_name TEXT NOT NULL
);
