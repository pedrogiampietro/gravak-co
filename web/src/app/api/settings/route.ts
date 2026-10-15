import { connection, NextResponse } from "next/server";
import { pool } from "@/lib/db";
import { currentAccount } from "@/lib/session";

/**
 * The signed-in account's settings files for the browser client
 * (client/crates/sandbox/src/saved.rs): `{ files: { "options.ini": "..." } }`.
 */
export async function GET() {
  await connection();
  const account = await currentAccount();
  if (!account) return NextResponse.json({ error: "sign in" }, { status: 401 });
  try {
    const { rows } = await pool.query<{ name: string; body: string }>(
      "SELECT name, body FROM account_settings WHERE account_id = $1",
      [account.id],
    );
    const files = Object.fromEntries(rows.map((row) => [row.name, row.body]));
    return NextResponse.json({ files }, { headers: { "Cache-Control": "no-store" } });
  } catch (error) {
    console.error("settings read failed", error);
    return NextResponse.json({ error: "failed" }, { status: 502 });
  }
}
