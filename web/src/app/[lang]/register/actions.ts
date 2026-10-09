"use server";

import bcrypt from "bcryptjs";
import { z } from "zod";
import { trackEvent } from "@/lib/analytics";
import { pool } from "@/lib/db";
import { clientAddress } from "@/lib/request";
import { passedTurnstile } from "@/lib/turnstile";

export type RegisterState =
  | { status: "idle" }
  | { status: "created"; username: string }
  | {
      status: "error";
      errors: Partial<Record<"username" | "password" | "confirm" | "email" | "form", ErrorCode>>;
      values: { username: string; email: string };
    };

export type ErrorCode = "username" | "password" | "confirm" | "email" | "taken" | "offline" | "tooMany" | "captcha";

// What the game client takes (launcher.rs `valid_account`, `valid_password`):
// usernames up to 15, passwords of up to 15 printable ASCII characters, no
// spaces. Its login carries 16 bytes of password, so a longer one could never
// log in. Emails up to 64, as the `accounts` table.
const schema = z
  .object({
    username: z.string().regex(/^[A-Za-z0-9]{4,15}$/),
    password: z.string().regex(/^[\x21-\x7e]{6,15}$/),
    confirm: z.string(),
    email: z.union([z.literal(""), z.string().email().max(64)]),
  })
  .refine((form) => form.password === form.confirm, { path: ["confirm"] });

// A few accounts per address in a window, kept in memory: enough to stop a
// script, nothing to configure.
const WINDOW_MS = 10 * 60 * 1000;
const PER_WINDOW = 5;
const recent = new Map<string, number[]>();

function allowed(address: string): boolean {
  const now = Date.now();
  const times = (recent.get(address) ?? []).filter((time) => now - time < WINDOW_MS);
  if (times.length >= PER_WINDOW) {
    recent.set(address, times);
    return false;
  }
  times.push(now);
  recent.set(address, times);
  return true;
}

export async function register(_previous: RegisterState, form: FormData): Promise<RegisterState> {
  const input = {
    username: String(form.get("username") ?? "").trim(),
    password: String(form.get("password") ?? ""),
    confirm: String(form.get("confirm") ?? ""),
    email: String(form.get("email") ?? "").trim(),
  };
  const values = { username: input.username, email: input.email };
  const parsed = schema.safeParse(input);
  if (!parsed.success) {
    const errors: Extract<RegisterState, { status: "error" }>["errors"] = {};
    for (const issue of parsed.error.issues) {
      const field = issue.path[0] as "username" | "password" | "confirm" | "email";
      errors[field] ??= field;
    }
    return { status: "error", errors, values };
  }

  const address = await clientAddress();
  if (!(await passedTurnstile(form.get("cf-turnstile-response"), address))) {
    return { status: "error", errors: { form: "captcha" }, values };
  }
  if (!allowed(address)) {
    return { status: "error", errors: { form: "tooMany" }, values };
  }

  try {
    // The game compares usernames exactly; refuse ones differing only in case
    // so nobody can pass for someone else.
    const taken = await pool.query("SELECT 1 FROM accounts WHERE lower(username) = lower($1)", [parsed.data.username]);
    if (taken.rowCount) {
      return { status: "error", errors: { username: "taken" }, values };
    }
    // Hashed as the server's `Account::create` does (bcrypt, cost 12).
    const hash = await bcrypt.hash(parsed.data.password, 12);
    await pool.query("INSERT INTO accounts (username, password, email) VALUES ($1, $2, $3)", [
      parsed.data.username,
      hash,
      parsed.data.email || null,
    ]);
  } catch (error) {
    if ((error as { code?: string }).code === "23505") {
      return { status: "error", errors: { username: "taken" }, values };
    }
    console.error("account creation failed", error);
    return { status: "error", errors: { form: "offline" }, values };
  }
  await trackEvent("cadastro");
  return { status: "created", username: parsed.data.username };
}
