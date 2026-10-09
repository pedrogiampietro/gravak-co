import "server-only";
import { headers } from "next/headers";

/**
 * The visitor's address. nginx sets X-Forwarded-For to the peer it accepted
 * the connection from (not appending to the visitor's own header, which
 * anyone can forge); "local" when the site runs without it.
 */
export async function clientAddress(): Promise<string> {
  return (await headers()).get("x-forwarded-for")?.split(",")[0]?.trim() || "local";
}
