import "server-only";

/** A settings file's name, as the client's `saved.rs` keeps them. */
export const SETTINGS_NAME = /^[a-z0-9_]{1,40}\.ini$/;
/** The most text a settings file may hold (as the table's CHECK). */
export const SETTINGS_MAX_BYTES = 65_536;
/** The most files an account may keep. */
export const SETTINGS_MAX_FILES = 200;
