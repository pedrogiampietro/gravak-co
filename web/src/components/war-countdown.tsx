"use client";

import { useEffect, useState } from "react";

const WAR_START_HOUR = 20;
const WAR_END_HOUR = 22;
const SATURDAY = 6;

/** Milliseconds to the next war (0 while one runs), the server's clock being UTC + `offset` hours. */
export function untilWar(now: number, offset: number): number {
  const server = new Date(now + offset * 3_600_000);
  const day = server.getUTCDay();
  const hour = server.getUTCHours();
  if (day === SATURDAY && hour >= WAR_START_HOUR && hour < WAR_END_HOUR) return 0;
  const next = new Date(server);
  next.setUTCHours(WAR_START_HOUR, 0, 0, 0);
  let days = (SATURDAY - day + 7) % 7;
  if (days === 0 && hour >= WAR_START_HOUR) days = 7;
  next.setUTCDate(next.getUTCDate() + days);
  return next.getTime() - server.getTime();
}

type Labels = { title: string; now: string; days: string; hours: string; minutes: string; seconds: string };

export function WarCountdown({ offset, labels }: { offset: number; labels: Labels }) {
  // Counted in the browser only: the server's render would be a second stale.
  const [left, setLeft] = useState<number | null>(null);
  useEffect(() => {
    const update = () => setLeft(untilWar(Date.now(), offset));
    update();
    const timer = setInterval(update, 1_000);
    return () => clearInterval(timer);
  }, [offset]);

  const total = Math.max(0, Math.floor((left ?? 0) / 1000));
  const parts: [number, string][] = [
    [Math.floor(total / 86_400), labels.days],
    [Math.floor((total % 86_400) / 3_600), labels.hours],
    [Math.floor((total % 3_600) / 60), labels.minutes],
    [total % 60, labels.seconds],
  ];
  return (
    <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1">
      <span className="text-sm text-slate-blue">{labels.title}</span>
      {left === 0 ? (
        <span className="font-display text-xl font-bold text-gold">{labels.now}</span>
      ) : (
        <span className="font-display text-xl font-bold text-parchment tabular-nums" aria-live="off">
          {left === null
            ? "— — — —"
            : parts.map(([value, unit]) => (
                <span key={unit} className="mr-2">
                  {String(value).padStart(2, "0")}
                  <span className="ml-0.5 text-sm text-slate-blue">{unit}</span>
                </span>
              ))}
        </span>
      )}
    </div>
  );
}
