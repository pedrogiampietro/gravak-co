import "server-only";

/**
 * At most `limit` uses per key in `windowMs`, kept in memory (one site
 * process): enough to stop a script, nothing to configure.
 */
export function rateLimiter(limit: number, windowMs: number): (key: string) => boolean {
  const recent = new Map<string, number[]>();
  return (key) => {
    const now = Date.now();
    const times = (recent.get(key) ?? []).filter((time) => now - time < windowMs);
    const allowed = times.length < limit;
    if (allowed) times.push(now);
    recent.set(key, times);
    return allowed;
  };
}
