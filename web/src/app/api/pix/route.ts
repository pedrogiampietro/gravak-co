import { connection, NextResponse, type NextRequest } from "next/server";
import { pool } from "@/lib/db";
import { createPix } from "@/lib/mercadopago";
import { createOrder, getPackages } from "@/lib/orders";
import { rateLimiter } from "@/lib/rate-limit";
import { clientAddress } from "@/lib/request";

// The client's CP shop: Pix only. The CPs go to the character named, so a
// charge for someone else's character is a gift, paid by whoever pays it;
// the limits below only keep scripts from piling up charges.
const perAddress = rateLimiter(6, 10 * 60 * 1000);
const PENDING_PER_CHARACTER = 3;

/** The packages on sale: `{ packages: [{ id, priceCents, cps }] }`. */
export async function GET() {
  await connection();
  try {
    return NextResponse.json({ packages: await getPackages() });
  } catch (error) {
    console.error("packages failed", error);
    return NextResponse.json({ error: "offline" }, { status: 503 });
  }
}

/**
 * A Pix charge: `{ character, package }` in, the order with its Pix code
 * and QR image (PNG, base64) out. The client then polls /api/pix/<order>.
 */
export async function POST(request: NextRequest) {
  const input = (await request.json().catch(() => null)) as { character?: unknown; package?: unknown } | null;
  const name = typeof input?.character === "string" ? input.character.trim() : "";
  const packageId = typeof input?.package === "number" ? input.package : NaN;
  if (!name || name.length > 16 || !Number.isInteger(packageId)) {
    return NextResponse.json({ error: "invalid" }, { status: 400 });
  }
  if (!perAddress(await clientAddress())) {
    return NextResponse.json({ error: "tooMany" }, { status: 429 });
  }

  try {
    const { rows } = await pool.query<{ character_id: string; account_id: string; name: string; email: string | null; pending: number }>(
      `SELECT c.character_id, c.account_id, c.name, a.email,
              (SELECT count(*)::int FROM payment_orders o
                WHERE o.character_id = c.character_id AND o.status = 'pending' AND o.expires_at > now()) AS pending
         FROM characters c JOIN accounts a ON a.account_id = c.account_id
        WHERE c.name = $1`,
      [name],
    );
    const character = rows[0];
    if (!character) return NextResponse.json({ error: "noCharacter" }, { status: 404 });
    if (character.pending >= PENDING_PER_CHARACTER) return NextResponse.json({ error: "tooMany" }, { status: 429 });

    const order = await createOrder({
      accountId: Number(character.account_id),
      characterId: Number(character.character_id),
      characterName: character.name,
      packageId,
      channel: "client",
    });
    if (!order) return NextResponse.json({ error: "noPackage" }, { status: 404 });

    // Mercado Pago asks for the payer's email; the account's when it has one.
    const email = character.email || `cps+${character.account_id}@gravak.fun`;
    let pix;
    try {
      pix = await createPix(order, email);
    } catch (error) {
      await pool.query("UPDATE payment_orders SET status = 'error' WHERE order_id = $1::uuid", [order.orderId]);
      throw error;
    }
    await pool.query(
      "UPDATE payment_orders SET mp_payment_id = $2, pix_code = $3, pix_qr_base64 = $4, expires_at = $5 WHERE order_id = $1::uuid",
      [order.orderId, pix.paymentId, pix.code, pix.qrBase64, pix.expiresAt],
    );
    return NextResponse.json({
      order: order.orderId,
      character: order.characterName,
      cps: order.cps,
      priceCents: order.priceCents,
      code: pix.code,
      qr: pix.qrBase64,
      expiresAt: pix.expiresAt.toISOString(),
    });
  } catch (error) {
    console.error("pix charge failed", error);
    return NextResponse.json({ error: "offline" }, { status: 502 });
  }
}
