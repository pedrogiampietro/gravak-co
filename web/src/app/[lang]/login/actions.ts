"use server";

import bcrypt from "bcryptjs";
import { redirect } from "next/navigation";
import { pool } from "@/lib/db";
import { rateLimiter } from "@/lib/rate-limit";
import { clientAddress } from "@/lib/request";
import { endSession, startSession } from "@/lib/session";
import { passedTurnstile } from "@/lib/turnstile";

export type LoginError = "invalid" | "banned" | "captcha" | "tooMany" | "offline";
export type LoginState = { status: "idle" } | { status: "error"; error: LoginError; username: string };

const perAddress = rateLimiter(10, 10 * 60 * 1000);
const perUsername = rateLimiter(10, 10 * 60 * 1000);
// A bcrypt hash of a random password, compared against when there is no such account.
const NO_ACCOUNT = "$2b$12$ty2gr6q7wJ6dBWzknzKcauYrc2h1Z3Uk/gk0vhjFCC6WxEyNmo7Qq";

/** Where to go after signing in: a page of this site in the same language, else the shop. */
function destination(lang: string, next: string): string {
  return next.startsWith(`/${lang}/`) && !next.startsWith("//") ? next : `/${lang}/shop`;
}

export async function login(_previous: LoginState, form: FormData): Promise<LoginState> {
  const lang = form.get("lang") === "en" ? "en" : "pt";
  const username = String(form.get("username") ?? "").trim();
  const password = String(form.get("password") ?? "");
  if (!username || !password || username.length > 16 || password.length > 72) {
    return { status: "error", error: "invalid", username };
  }
  const address = await clientAddress();
  if (!(await passedTurnstile(form.get("cf-turnstile-response"), address))) {
    return { status: "error", error: "captcha", username };
  }
  if (!perAddress(address) || !perUsername(username.toLowerCase())) {
    return { status: "error", error: "tooMany", username };
  }

  let account;
  try {
    // The game compares usernames exactly, and so does this.
    const { rows } = await pool.query<{ account_id: string; password: string; banned: boolean }>(
      "SELECT account_id, password, COALESCE(banned_until > now(), false) AS banned FROM accounts WHERE username = $1",
      [username],
    );
    account = rows[0];
    // Compared even without the account, so the answer takes as long.
    const matches = await bcrypt.compare(password, account?.password ?? NO_ACCOUNT);
    if (!account || !matches) return { status: "error", error: "invalid", username };
    if (account.banned) return { status: "error", error: "banned", username };
  } catch (error) {
    console.error("login failed", error);
    return { status: "error", error: "offline", username };
  }
  await startSession(Number(account.account_id));
  redirect(destination(lang, String(form.get("next") ?? "")));
}

export async function logout(form: FormData): Promise<void> {
  await endSession();
  redirect(`/${form.get("lang") === "en" ? "en" : "pt"}/shop`);
}
