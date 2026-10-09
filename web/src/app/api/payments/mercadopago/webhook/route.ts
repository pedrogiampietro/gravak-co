import { NextResponse, type NextRequest } from "next/server";
import { getPayment, signedByMercadoPago } from "@/lib/mercadopago";
import { applyPayment } from "@/lib/orders";

/**
 * Mercado Pago's notifications (notification_url of each preference and Pix
 * payment). Only the payment's id is taken from them: the payment itself is
 * read back from the API before the order changes. A failure answers 500,
 * so Mercado Pago tries again.
 */
export async function POST(request: NextRequest) {
  const params = request.nextUrl.searchParams;
  const body = (await request.json().catch(() => ({}))) as { type?: string; topic?: string; data?: { id?: string | number } };
  const type = body.type ?? body.topic ?? params.get("type") ?? params.get("topic");
  const id = String(params.get("data.id") ?? body.data?.id ?? params.get("id") ?? "");
  if (type !== "payment" || !/^\d+$/.test(id)) {
    return NextResponse.json({ ignored: true });
  }
  if (!signedByMercadoPago(request.headers.get("x-signature"), request.headers.get("x-request-id"), id)) {
    console.warn("mercado pago notification with a bad signature", { id });
    return NextResponse.json({ error: "bad signature" }, { status: 401 });
  }
  try {
    await applyPayment(await getPayment(id));
  } catch (error) {
    console.error("mercado pago notification failed", { id, error });
    return NextResponse.json({ error: "failed" }, { status: 500 });
  }
  return NextResponse.json({ ok: true });
}
