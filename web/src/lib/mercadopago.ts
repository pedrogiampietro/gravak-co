import "server-only";
import { createHmac, timingSafeEqual } from "node:crypto";

// Mercado Pago's REST API: Checkout Pro (card, Pix and the rest, on Mercado
// Pago's page) for the site and Pix payments for the client. The game server
// credits the CPs once an order is approved (server/.../payments.rs).
const API = "https://api.mercadopago.com";

export const SITE_URL = (process.env.SITE_URL ?? "http://localhost:3000").replace(/\/$/, "");
const WEBHOOK_URL = `${SITE_URL}/api/payments/mercadopago/webhook`;
// How long a Pix code can be paid.
const PIX_MINUTES = 30;

export type Payment = {
  id: number;
  status: string;
  external_reference: string | null;
  transaction_amount: number;
  currency_id: string;
};

async function call<T>(path: string, init: { method: "GET" | "POST"; body?: unknown; idempotencyKey?: string }): Promise<T> {
  const token = process.env.MP_ACCESS_TOKEN;
  if (!token) throw new Error("MP_ACCESS_TOKEN is not set");
  const response = await fetch(`${API}${path}`, {
    method: init.method,
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/json",
      ...(init.idempotencyKey ? { "X-Idempotency-Key": init.idempotencyKey } : {}),
    },
    body: init.body === undefined ? undefined : JSON.stringify(init.body),
    cache: "no-store",
    signal: AbortSignal.timeout(15_000),
  });
  if (!response.ok) {
    throw new Error(`Mercado Pago ${init.method} ${path}: ${response.status} ${await response.text()}`);
  }
  return (await response.json()) as T;
}

type OrderToPay = { orderId: string; priceCents: number; cps: number; characterName: string };

const title = (order: OrderToPay) => `${order.cps} CPs - Gravak (${order.characterName})`;

/** A Checkout Pro preference for the order; the buyer pays at its `init_point`. */
export async function createPreference(order: OrderToPay, lang: string): Promise<string> {
  const back = `${SITE_URL}/${lang}/shop/result`;
  const preference = await call<{ id: string; init_point: string }>("/checkout/preferences", {
    method: "POST",
    idempotencyKey: order.orderId,
    body: {
      items: [
        {
          id: `cps-${order.cps}`,
          title: title(order),
          quantity: 1,
          currency_id: "BRL",
          unit_price: order.priceCents / 100,
        },
      ],
      external_reference: order.orderId,
      notification_url: WEBHOOK_URL,
      back_urls: { success: back, pending: back, failure: back },
      auto_return: "approved",
      statement_descriptor: "GRAVAK",
    },
  });
  return preference.init_point;
}

export type Pix = { paymentId: number; code: string; qrBase64: string; expiresAt: Date };

/** A Pix payment for the order: the copy-and-paste code and its QR image. */
export async function createPix(order: OrderToPay, email: string): Promise<Pix> {
  const expiresAt = new Date(Date.now() + PIX_MINUTES * 60 * 1000);
  const payment = await call<{
    id: number;
    point_of_interaction?: { transaction_data?: { qr_code?: string; qr_code_base64?: string } };
  }>("/v1/payments", {
    method: "POST",
    idempotencyKey: order.orderId,
    body: {
      transaction_amount: order.priceCents / 100,
      description: title(order),
      payment_method_id: "pix",
      payer: { email },
      external_reference: order.orderId,
      notification_url: WEBHOOK_URL,
      date_of_expiration: expiresAt.toISOString().replace("Z", "-00:00"),
    },
  });
  const data = payment.point_of_interaction?.transaction_data;
  if (!data?.qr_code || !data.qr_code_base64) throw new Error(`Mercado Pago payment ${payment.id} came without a Pix code`);
  return { paymentId: payment.id, code: data.qr_code, qrBase64: data.qr_code_base64, expiresAt };
}

export async function getPayment(id: string | number): Promise<Payment> {
  return call<Payment>(`/v1/payments/${encodeURIComponent(String(id))}`, { method: "GET" });
}

/** A chargeback (a buyer's dispute with the card issuer) and the payments it is about. */
export async function getChargeback(id: string | number): Promise<{ payments: (string | number)[] }> {
  const chargeback = await call<{ payments?: (string | number)[] }>(`/v1/chargebacks/${encodeURIComponent(String(id))}`, { method: "GET" });
  return { payments: chargeback.payments ?? [] };
}

/**
 * Whether a notification carries Mercado Pago's signature: x-signature is
 * "ts=<ts>,v1=<hmac>", the HMAC-SHA256 (MP_WEBHOOK_SECRET) of
 * "id:<data.id>;request-id:<x-request-id>;ts:<ts>;". Without the secret,
 * nothing to check: the payment is read back from the API either way.
 */
export function signedByMercadoPago(signature: string | null, requestId: string | null, dataId: string): boolean {
  const secret = process.env.MP_WEBHOOK_SECRET;
  if (!secret) return true;
  if (!signature) return false;
  const parts = Object.fromEntries(signature.split(",").map((part) => part.trim().split("=", 2) as [string, string]));
  if (!parts.ts || !parts.v1) return false;
  const id = /^[a-z0-9]+$/i.test(dataId) ? dataId.toLowerCase() : dataId;
  const manifest = `id:${id};${requestId ? `request-id:${requestId};` : ""}ts:${parts.ts};`;
  const expected = Buffer.from(createHmac("sha256", secret).update(manifest).digest("hex"));
  const given = Buffer.from(parts.v1);
  return given.length === expected.length && timingSafeEqual(given, expected);
}
