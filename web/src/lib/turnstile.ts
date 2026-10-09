import "server-only";

const VERIFY_URL = "https://challenges.cloudflare.com/turnstile/v0/siteverify";

/**
 * Whether the form's Cloudflare Turnstile token (`cf-turnstile-response`) is
 * a real one, asked of Cloudflare. A token works once. Without
 * TURNSTILE_SECRET_KEY it passes while developing and fails in production.
 */
export async function passedTurnstile(token: FormDataEntryValue | null, address: string): Promise<boolean> {
  const secret = process.env.TURNSTILE_SECRET_KEY;
  if (!secret) return process.env.NODE_ENV !== "production";
  if (typeof token !== "string" || !token || token.length > 2048) return false;

  const body = new URLSearchParams({ secret, response: token });
  if (address !== "local") body.set("remoteip", address);
  try {
    const response = await fetch(VERIFY_URL, {
      method: "POST",
      body,
      cache: "no-store",
      signal: AbortSignal.timeout(10_000),
    });
    const result = (await response.json()) as { success?: boolean };
    return result.success === true;
  } catch (error) {
    console.error("turnstile verification failed", error);
    return false;
  }
}
