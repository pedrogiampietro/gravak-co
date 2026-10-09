import { connection, NextResponse, type NextRequest } from "next/server";
import { pool } from "@/lib/db";
import { getPayment } from "@/lib/mercadopago";
import { applyPayment } from "@/lib/orders";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/**
 * Where a client's Pix order stands: `{ status }`, one of "pending",
 * "approved" (paid, the CPs on their way), "credited" (the CPs are on the
 * character), "expired" or Mercado Pago's own (rejected, cancelled...).
 * While pending it asks Mercado Pago too, so a lost notification never
 * leaves a paid order waiting.
 */
export async function GET(_request: NextRequest, { params }: { params: Promise<{ order: string }> }) {
  await connection();
  const { order } = await params;
  if (!UUID.test(order)) return NextResponse.json({ error: "invalid" }, { status: 400 });
  try {
    const read = () =>
      pool.query<{ status: string; credited: boolean; expired: boolean; mp_payment_id: string | null }>(
        `SELECT status, credited_at IS NOT NULL AS credited, expires_at < now() AS expired, mp_payment_id
           FROM payment_orders WHERE order_id = $1::uuid AND channel = 'client'`,
        [order],
      );
    let row = (await read()).rows[0];
    if (!row) return NextResponse.json({ error: "noOrder" }, { status: 404 });
    if (row.status === "pending" && row.mp_payment_id) {
      await applyPayment(await getPayment(row.mp_payment_id));
      row = (await read()).rows[0] ?? row;
    }
    const status = row.credited ? "credited" : row.status === "pending" && row.expired ? "expired" : row.status;
    return NextResponse.json({ status });
  } catch (error) {
    console.error("pix status failed", error);
    return NextResponse.json({ error: "offline" }, { status: 502 });
  }
}
