import { connection, type NextRequest, NextResponse } from "next/server";
import { pool } from "@/lib/db";
import { currentAccount } from "@/lib/session";
import { SETTINGS_MAX_BYTES, SETTINGS_MAX_FILES, SETTINGS_NAME } from "@/lib/settings";

/**
 * Keeps one settings file of the browser client for the signed-in account;
 * the body is the file's text. POST, so a page closing can still send it
 * (fetch with keepalive).
 */
export async function POST(request: NextRequest, { params }: { params: Promise<{ name: string }> }) {
  await connection();
  const name = (await params).name;
  if (!SETTINGS_NAME.test(name)) return NextResponse.json({ error: "invalid name" }, { status: 400 });
  const account = await currentAccount();
  if (!account) return NextResponse.json({ error: "sign in" }, { status: 401 });
  const body = await request.text();
  if (Buffer.byteLength(body) > SETTINGS_MAX_BYTES) return NextResponse.json({ error: "too large" }, { status: 413 });
  try {
    // A new name only while the account keeps fewer than the most files.
    const { rowCount } = await pool.query(
      `INSERT INTO account_settings (account_id, name, body)
       SELECT $1, $2, $3
       WHERE EXISTS (SELECT 1 FROM account_settings WHERE account_id = $1 AND name = $2)
          OR (SELECT count(*) FROM account_settings WHERE account_id = $1) < $4
       ON CONFLICT (account_id, name) DO UPDATE SET body = EXCLUDED.body, updated_at = now()`,
      [account.id, name, body, SETTINGS_MAX_FILES],
    );
    if (!rowCount) return NextResponse.json({ error: "too many files" }, { status: 413 });
    return new NextResponse(null, { status: 204 });
  } catch (error) {
    console.error("settings write failed", error);
    return NextResponse.json({ error: "failed" }, { status: 502 });
  }
}
