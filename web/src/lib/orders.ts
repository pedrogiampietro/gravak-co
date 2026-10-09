import "server-only";
import { pool } from "@/lib/db";
import type { Payment } from "@/lib/mercadopago";

// The CP orders (`payment_orders`, server migration 90). The site creates
// them and records Mercado Pago's status; the game server credits approved
// ones (payments.rs) and stamps `credited_at`.

export type Package = { id: number; priceCents: number; cps: number };

export async function getPackages(): Promise<Package[]> {
  const { rows } = await pool.query<{ package_id: number; price_cents: number; cps: number }>(
    "SELECT package_id, price_cents, cps FROM cp_packages WHERE active ORDER BY price_cents",
  );
  return rows.map((row) => ({ id: row.package_id, priceCents: row.price_cents, cps: row.cps }));
}

export type NewOrder = { orderId: string; priceCents: number; cps: number; characterName: string };

/** An order for the package, its price and CPs copied; null for a package not on sale. */
export async function createOrder(input: {
  accountId: number;
  characterId: number;
  characterName: string;
  packageId: number;
  channel: "site" | "client";
}): Promise<NewOrder | null> {
  const { rows } = await pool.query<{ order_id: string; price_cents: number; cps: number }>(
    `INSERT INTO payment_orders (account_id, character_id, character_name, package_id, price_cents, cps, channel)
     SELECT $1, $2, $3, package_id, price_cents, cps, $5 FROM cp_packages WHERE package_id = $4 AND active
     RETURNING order_id::text, price_cents, cps`,
    [input.accountId, input.characterId, input.characterName, input.packageId, input.channel],
  );
  const row = rows[0];
  return row ? { orderId: row.order_id, priceCents: row.price_cents, cps: row.cps, characterName: input.characterName } : null;
}

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/**
 * Records Mercado Pago's status of a payment on its order (its
 * external_reference). A payment for another amount or currency is ignored,
 * and once an order is approved only that payment can change it (a refund,
 * a chargeback): Checkout Pro may try several payments for one order.
 */
export async function applyPayment(payment: Payment): Promise<void> {
  const orderId = payment.external_reference ?? "";
  if (!UUID.test(orderId)) return;
  const amountCents = Math.round(payment.transaction_amount * 100);
  const { rowCount } = await pool.query(
    `UPDATE payment_orders
        SET status = $2,
            mp_payment_id = $3,
            paid_at = CASE WHEN $2 = 'approved' THEN COALESCE(paid_at, now()) ELSE paid_at END
      WHERE order_id = $1::uuid AND price_cents = $4 AND $5 = 'BRL'
        AND (status <> 'approved' OR mp_payment_id = $3)`,
    [orderId, payment.status, payment.id, amountCents, payment.currency_id],
  );
  if (!rowCount) {
    console.warn("payment not applied", { orderId, payment: payment.id, status: payment.status, amountCents });
  }
}

export type OrderSummary = {
  orderId: string;
  characterName: string;
  cps: number;
  priceCents: number;
  status: string;
  credited: boolean;
  createdAt: Date;
};

export async function getAccountOrders(accountId: number): Promise<OrderSummary[]> {
  const { rows } = await pool.query<{
    order_id: string;
    character_name: string;
    cps: number;
    price_cents: number;
    status: string;
    credited: boolean;
    created_at: Date;
  }>(
    `SELECT order_id::text, character_name, cps, price_cents, status, credited_at IS NOT NULL AS credited, created_at
       FROM payment_orders WHERE account_id = $1 AND (status <> 'pending' OR created_at > now() - interval '1 day')
      ORDER BY created_at DESC LIMIT 20`,
    [accountId],
  );
  return rows.map((row) => ({
    orderId: row.order_id,
    characterName: row.character_name,
    cps: row.cps,
    priceCents: row.price_cents,
    status: row.status,
    credited: row.credited,
    createdAt: row.created_at,
  }));
}
