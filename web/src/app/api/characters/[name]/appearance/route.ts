import { connection, type NextRequest, NextResponse } from "next/server";
import { armorySheet, readSheet } from "@/lib/armory";

/**
 * A character's look turned all the way around: one PNG of frames side by
 * side (`SHEET` in lib/armory), for the Armory. 404 when there is no such
 * character or the server does not draw heroes (no ARMORY_BIN).
 */
export async function GET(request: NextRequest, { params }: { params: Promise<{ name: string }> }) {
  await connection();
  const name = decodeURIComponent((await params).name);
  if (name.length === 0 || name.length > 16) return NextResponse.json({ error: "invalid" }, { status: 400 });
  try {
    const sheet = await armorySheet(name);
    if (!sheet) return NextResponse.json({ error: "none" }, { status: 404 });
    const etag = `"${sheet.key}"`;
    const headers = { ETag: etag, "Cache-Control": "public, max-age=300, stale-while-revalidate=86400" };
    if (request.headers.get("if-none-match") === etag) return new NextResponse(null, { status: 304, headers });
    const body = await readSheet(sheet.file);
    return new NextResponse(new Uint8Array(body), { headers: { ...headers, "Content-Type": "image/png" } });
  } catch (error) {
    console.error("armory drawing failed", error);
    return NextResponse.json({ error: "failed" }, { status: 502 });
  }
}
