import "server-only";
import { headers } from "next/headers";

// Umami (gravak.fun/stats, the gravak-umami service). The browser loads its
// tracker for page views and clicks (layout.tsx); what matters most (an
// account made, a download, an order) is sent from here too, so a blocked
// tracker does not hide it. Sent with the visitor's address and browser, it
// lands in the same visit as their page views.
const WEBSITE_ID = process.env.NEXT_PUBLIC_UMAMI_WEBSITE_ID;
const COLLECT_URL = process.env.UMAMI_COLLECT_URL ?? "http://127.0.0.1:3001/stats/api/send";
const SITE_URL = (process.env.SITE_URL ?? "http://localhost:3000").replace(/\/$/, "");

/** Records the event in Umami; never throws nor waits long, the visitor's request comes first. */
export async function trackEvent(name: string, data?: Record<string, string | number>): Promise<void> {
  if (!WEBSITE_ID) return;
  try {
    const request = await headers();
    const page = new URL(request.get("referer") ?? SITE_URL, SITE_URL);
    await fetch(COLLECT_URL, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "User-Agent": request.get("user-agent") ?? "",
        "X-Forwarded-For": request.get("x-forwarded-for") ?? "",
        "Accept-Language": request.get("accept-language") ?? "",
      },
      body: JSON.stringify({
        type: "event",
        payload: {
          website: WEBSITE_ID,
          hostname: new URL(SITE_URL).hostname,
          url: page.pathname,
          language: request.get("accept-language")?.split(",")[0] ?? "",
          name,
          data,
        },
      }),
      cache: "no-store",
      signal: AbortSignal.timeout(3_000),
    });
  } catch (error) {
    console.warn("analytics event not sent", { name, error: String(error) });
  }
}
