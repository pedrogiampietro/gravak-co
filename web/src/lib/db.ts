import "server-only";
import { Pool } from "pg";

// One pool for the process: the dev server reloads modules, so keep it on
// globalThis rather than opening a new one on every change.
const globalForPool = globalThis as unknown as { gravakPool?: Pool };

export const pool =
  globalForPool.gravakPool ??
  new Pool({
    connectionString: process.env.DATABASE_URL ?? "postgres://gravak:gravak@localhost:5432/gravak",
    max: 5,
    connectionTimeoutMillis: 3_000,
  });

if (process.env.NODE_ENV !== "production") globalForPool.gravakPool = pool;
