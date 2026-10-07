import "server-only";
import { Socket } from "node:net";
import { pool } from "./db";

export type ServerStatus = {
  up: boolean;
  /** Characters in the world; null when the server does not publish it. */
  online: number | null;
  record: number | null;
};

/** Whether something answers on `port` within a second. */
function answers(host: string, port: number): Promise<boolean> {
  return new Promise((resolve) => {
    const socket = new Socket();
    const done = (up: boolean) => {
      socket.destroy();
      resolve(up);
    };
    socket.setTimeout(1_000);
    socket.once("connect", () => done(true));
    socket.once("timeout", () => done(false));
    socket.once("error", () => done(false));
    socket.connect(port, host);
  });
}

// The game server writes `server_status` every 30 seconds; older than this,
// it is down.
const STALE_SECONDS = 90;

async function read(): Promise<ServerStatus> {
  try {
    const { rows } = await pool.query<{ online: number; record: number; age: string }>(
      "SELECT online, record, EXTRACT(EPOCH FROM now())::BIGINT - updated_at AS age FROM server_status WHERE id = 1",
    );
    if (rows[0]) {
      const up = Number(rows[0].age) <= STALE_SECONDS;
      return { up, online: up ? rows[0].online : 0, record: rows[0].record };
    }
  } catch {
    // No table yet (a server from before migration 76): ask the ports.
  }
  const host = process.env.GAME_HOST ?? "127.0.0.1";
  const [auth, game] = await Promise.all([
    answers(host, Number(process.env.AUTH_PORT ?? 9958)),
    answers(host, Number(process.env.GAME_PORT ?? 5816)),
  ]);
  return { up: auth && game, online: null, record: null };
}

// Asked at most every 15 seconds, however many visitors there are.
const TTL_MS = 15_000;
let last: { at: number; status: Promise<ServerStatus> } | null = null;

export function serverStatus(): Promise<ServerStatus> {
  if (last && Date.now() - last.at < TTL_MS) return last.status;
  last = { at: Date.now(), status: read() };
  return last.status;
}
