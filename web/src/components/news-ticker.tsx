import Link from "next/link";
import { type Category, type Post, posts } from "@/content/news";
import type { Dictionary, Locale } from "@/i18n/dictionaries";
import { cn } from "@/lib/utils";

/** Each kind of news keeps its colour, here and on the news pages. */
export const categoryTone: Record<Category, string> = {
  update: "text-gold",
  system: "text-heraldic",
  interface: "text-sky-300",
  site: "text-emerald-300",
  event: "text-orange-300",
  maintenance: "text-destructive",
};

export function formatDate(lang: Locale, iso: string, style: "short" | "long" = "short") {
  return new Intl.DateTimeFormat(lang === "pt" ? "pt-BR" : "en-US", {
    day: "2-digit",
    month: style === "short" ? "short" : "long",
    year: "numeric",
    timeZone: "UTC",
  }).format(new Date(`${iso}T00:00:00Z`));
}

/** The latest news, one line each: date, kind, title and the start of its summary. */
export function NewsTicker({ lang, dict, count = 5 }: { lang: Locale; dict: Dictionary; count?: number }) {
  const latest = posts.slice(0, count);
  return (
    <section aria-labelledby="news-ticker" className="overflow-hidden rounded-lg border border-gold/25 bg-night/80 backdrop-blur-sm">
      <div className="flex items-center justify-between border-b border-gold/20 bg-lapis/60 px-4 py-2.5">
        <h2 id="news-ticker" className="font-display text-base font-bold text-gold">
          {dict.news.title}
        </h2>
        <Link href={`/${lang}/news`} className="text-sm text-heraldic underline-offset-4 hover:underline">
          {dict.news.all}
        </Link>
      </div>
      <ul className="divide-y divide-white/5">
        {latest.map((post) => (
          <li key={post.slug}>
            <TickerRow post={post} lang={lang} dict={dict} />
          </li>
        ))}
      </ul>
    </section>
  );
}

function TickerRow({ post, lang, dict }: { post: Post; lang: Locale; dict: Dictionary }) {
  const text = post[lang];
  return (
    <Link
      href={`/${lang}/news/${post.slug}`}
      className="group flex items-center gap-3 px-4 py-2.5 text-[0.95rem] transition-colors hover:bg-lapis/50 focus-visible:bg-lapis/50"
    >
      <time dateTime={post.date} className="w-28 shrink-0 text-sm text-slate-blue tabular-nums">
        {formatDate(lang, post.date)}
      </time>
      <span className="min-w-0 flex-1 truncate">
        <span className={cn("font-bold", categoryTone[post.category])}>[{dict.news.categories[post.category]}]</span>{" "}
        <span className="font-medium text-parchment">{text.title}</span>
        <span className="hidden text-parchment/60 sm:inline"> — {text.summary}</span>
      </span>
      <svg aria-hidden="true" viewBox="0 0 24 24" className="size-4 shrink-0 fill-none stroke-slate-blue transition-transform group-hover:translate-x-0.5" strokeWidth="2">
        <path d="m9 6 6 6-6 6" />
      </svg>
    </Link>
  );
}
