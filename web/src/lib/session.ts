import "server-only";
import { createHmac, timingSafeEqual } from "node:crypto";
import { cookies } from "next/headers";
import { pool } from "@/lib/db";

const COOKIE = "gravak_session";
const DAYS = 7;

export type Account = { id: number; username: string; email: string | null };

function secret(): string {
  const value = process.env.SESSION_SECRET;
  if (value) return value;
  if (process.env.NODE_ENV === "production") throw new Error("SESSION_SECRET is not set");
  return "development-session-secret";
}

function sign(payload: string): string {
  return createHmac("sha256", secret()).update(payload).digest("base64url");
}

/** Signs the account in: a cookie "<account>.<expiry>.<hmac>" for a week. */
export async function startSession(accountId: number): Promise<void> {
  const expires = Math.floor(Date.now() / 1000) + DAYS * 24 * 60 * 60;
  const payload = `${accountId}.${expires}`;
  (await cookies()).set(COOKIE, `${payload}.${sign(payload)}`, {
    httpOnly: true,
    secure: process.env.NODE_ENV === "production",
    sameSite: "lax",
    path: "/",
    maxAge: DAYS * 24 * 60 * 60,
  });
}

export async function endSession(): Promise<void> {
  (await cookies()).delete(COOKIE);
}

/** The signed-in account, read again from the database: a ban signs it out. */
export async function currentAccount(): Promise<Account | null> {
  const value = (await cookies()).get(COOKIE)?.value;
  const [id, expires, mac] = value?.split(".") ?? [];
  if (!id || !expires || !mac) return null;
  const expected = Buffer.from(sign(`${id}.${expires}`));
  const given = Buffer.from(mac);
  if (given.length !== expected.length || !timingSafeEqual(given, expected)) return null;
  if (Number(expires) < Date.now() / 1000) return null;
  try {
    const { rows } = await pool.query<{ account_id: string; username: string; email: string | null }>(
      "SELECT account_id, username, email FROM accounts WHERE account_id = $1 AND (banned_until IS NULL OR banned_until < now())",
      [id],
    );
    const row = rows[0];
    return row ? { id: Number(row.account_id), username: row.username, email: row.email } : null;
  } catch (error) {
    console.error("session lookup failed", error);
    return null;
  }
}
