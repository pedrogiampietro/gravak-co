"use server";

import { redirect } from "next/navigation";
import { pool } from "@/lib/db";
import { createPreference } from "@/lib/mercadopago";
import { createOrder } from "@/lib/orders";
import { rateLimiter } from "@/lib/rate-limit";
import { currentAccount } from "@/lib/session";

export type BuyError = "character" | "package" | "offline";
export type BuyState = { status: "idle" } | { status: "error"; error: BuyError };

const perAccount = rateLimiter(10, 10 * 60 * 1000);

/** An order for one of the account's characters, then off to Mercado Pago's Checkout Pro. */
export async function buy(_previous: BuyState, form: FormData): Promise<BuyState> {
  const lang = form.get("lang") === "en" ? "en" : "pt";
  const account = await currentAccount();
  if (!account) redirect(`/${lang}/login?next=/${lang}/shop`);
  const characterId = Number(form.get("character"));
  const packageId = Number(form.get("package"));
  if (!Number.isInteger(characterId)) return { status: "error", error: "character" };
  if (!Number.isInteger(packageId)) return { status: "error", error: "package" };
  if (!perAccount(String(account.id))) return { status: "error", error: "offline" };

  let checkout: string;
  try {
    const { rows } = await pool.query<{ name: string }>("SELECT name FROM characters WHERE character_id = $1 AND account_id = $2", [
      characterId,
      account.id,
    ]);
    const character = rows[0];
    if (!character) return { status: "error", error: "character" };
    const order = await createOrder({ accountId: account.id, characterId, characterName: character.name, packageId, channel: "site" });
    if (!order) return { status: "error", error: "package" };
    try {
      checkout = await createPreference(order, lang);
    } catch (error) {
      await pool.query("UPDATE payment_orders SET status = 'error' WHERE order_id = $1::uuid", [order.orderId]);
      throw error;
    }
  } catch (error) {
    console.error("checkout failed", error);
    return { status: "error", error: "offline" };
  }
  redirect(checkout);
}
