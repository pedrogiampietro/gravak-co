import type { Locale } from "@/i18n/dictionaries";

const tag = (lang: Locale) => (lang === "pt" ? "pt-BR" : "en-US");

export function number(lang: Locale, value: number) {
  return new Intl.NumberFormat(tag(lang)).format(value);
}

/** A unix time as a date, in the server's zone (Brasília). */
export function date(lang: Locale, seconds: number) {
  return new Intl.DateTimeFormat(tag(lang), { day: "2-digit", month: "short", year: "numeric", timeZone: "America/Sao_Paulo" }).format(
    new Date(seconds * 1000),
  );
}

export function dateTime(lang: Locale, seconds: number) {
  return new Intl.DateTimeFormat(tag(lang), {
    day: "2-digit",
    month: "short",
    hour: "2-digit",
    minute: "2-digit",
    timeZone: "America/Sao_Paulo",
  }).format(new Date(seconds * 1000));
}

/** "3 hours ago", "yesterday". */
export function ago(lang: Locale, seconds: number, now = Date.now() / 1000) {
  const format = new Intl.RelativeTimeFormat(tag(lang), { numeric: "auto" });
  const elapsed = Math.max(0, now - seconds);
  if (elapsed < 3_600) return format.format(-Math.max(1, Math.round(elapsed / 60)), "minute");
  if (elapsed < 86_400) return format.format(-Math.round(elapsed / 3_600), "hour");
  return format.format(-Math.round(elapsed / 86_400), "day");
}
