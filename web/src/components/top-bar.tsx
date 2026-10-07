import Link from "next/link";
import { connection } from "next/server";
import { Suspense } from "react";
import type { Dictionary, Locale } from "@/i18n/dictionaries";
import { serverStatus } from "@/lib/status";
import { cn } from "@/lib/utils";
import { LanguageFlags } from "./language-flags";

const DISCORD = process.env.NEXT_PUBLIC_DISCORD_URL;
const INSTAGRAM = process.env.NEXT_PUBLIC_INSTAGRAM_URL;

/** The strip over the header: where to find us, who is in the world, the language. */
export function TopBar({ lang, dict }: { lang: Locale; dict: Dictionary }) {
  return (
    <div className="border-b border-white/5 bg-lapis/60 text-[0.85rem]">
      <div className="mx-auto flex h-9 max-w-6xl items-center gap-4 px-4 text-parchment/80 sm:px-6">
        {DISCORD && (
          <a href={DISCORD} className="hidden items-center gap-1.5 hover:text-parchment sm:inline-flex">
            <DiscordIcon />
            {dict.nav.discord}
          </a>
        )}
        {INSTAGRAM && (
          <a href={INSTAGRAM} className="hidden items-center gap-1.5 hover:text-parchment sm:inline-flex">
            <InstagramIcon />
            {dict.nav.instagram}
          </a>
        )}
        <Link href={`/${lang}/download`} className="hidden items-center gap-1.5 hover:text-parchment sm:inline-flex">
          <DownloadIcon />
          {dict.nav.download}
        </Link>
        <span className="hidden h-4 w-px bg-white/10 sm:block" aria-hidden="true" />
        <Suspense fallback={<Online up={null} label={dict.status.checking} />}>
          <LiveOnline dict={dict} />
        </Suspense>
        {/* Until the page's path is known, the flags lead to each home. */}
        <Suspense fallback={<LanguageFlags lang={lang} className="ml-auto" pathname={`/${lang}`} />}>
          <LanguageFlags lang={lang} className="ml-auto" />
        </Suspense>
      </div>
    </div>
  );
}

async function LiveOnline({ dict }: { dict: Dictionary }) {
  await connection();
  const status = await serverStatus();
  if (!status.up) return <Online up={false} label={dict.status.offline} />;
  if (status.online === null) return <Online up label={dict.status.online} />;
  return (
    <Online
      up
      label={
        <>
          <strong className="font-bold text-emerald-300 tabular-nums">{status.online}</strong> {dict.status.players}
          {status.record ? (
            <span className="hidden text-slate-blue md:inline">
              {" "}
              · {dict.status.record} <span className="tabular-nums">{status.record}</span>
            </span>
          ) : null}
        </>
      }
    />
  );
}

function Online({ up, label }: { up: boolean | null; label: React.ReactNode }) {
  return (
    <span className="inline-flex items-center gap-2" role="status">
      <span
        aria-hidden="true"
        className={cn(
          "size-2 rounded-full",
          up === null ? "bg-slate-blue/60" : up ? "bg-emerald-400 shadow-[0_0_8px_rgb(52_211_153/0.8)]" : "bg-destructive",
        )}
      />
      <span>{label}</span>
    </span>
  );
}

function DiscordIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 24 24" className="size-4 fill-current">
      <path d="M20.3 4.4A19.6 19.6 0 0 0 15.4 3l-.6 1.3a18 18 0 0 0-5.6 0L8.6 3a19.6 19.6 0 0 0-4.9 1.4C.6 9 0 13.5.3 18a19.8 19.8 0 0 0 6 3l1.3-2a12.7 12.7 0 0 1-2-1l.5-.4a14 14 0 0 0 11.8 0l.5.4a12.7 12.7 0 0 1-2 1l1.3 2a19.8 19.8 0 0 0 6-3c.5-5.2-.8-9.7-3.4-13.6ZM8.5 15.3c-1.2 0-2.1-1.1-2.1-2.4s.9-2.4 2.1-2.4 2.2 1.1 2.1 2.4c0 1.3-.9 2.4-2.1 2.4Zm7 0c-1.2 0-2.1-1.1-2.1-2.4s.9-2.4 2.1-2.4 2.2 1.1 2.1 2.4c0 1.3-.9 2.4-2.1 2.4Z" />
    </svg>
  );
}

function InstagramIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 24 24" className="size-4 fill-none stroke-current" strokeWidth="2">
      <rect x="3" y="3" width="18" height="18" rx="5" />
      <circle cx="12" cy="12" r="4" />
      <circle cx="17.5" cy="6.5" r="0.6" className="fill-current" />
    </svg>
  );
}

function DownloadIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 24 24" className="size-4 fill-none stroke-current" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 3v12m0 0-4-4m4 4 4-4M5 21h14" />
    </svg>
  );
}
