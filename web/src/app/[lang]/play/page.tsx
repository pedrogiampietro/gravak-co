import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { Suspense } from "react";
import { buttonVariants } from "@/components/ui/button";
import { type Dictionary, getDictionary, hasLocale, type Locale } from "@/i18n/dictionaries";
import { currentAccount } from "@/lib/session";
import { cn } from "@/lib/utils";
import { GameFrame } from "./game-frame";

// The client compiled to wasm (docs/web-spike.md), copied into public/play at
// deploy by client/tools/build-web.ps1. A frame of its own: the client takes
// the whole page it runs in. Its game files come from /game-files
// (client/crates/web-files), only with this site's session: so the page asks
// for it first.
const CLIENT = "/play/index.html";

export async function generateMetadata({ params }: PageProps<"/[lang]/play">): Promise<Metadata> {
  const { lang } = await params;
  return hasLocale(lang) ? { title: getDictionary(lang).play.title } : {};
}

export default async function PlayPage({ params }: PageProps<"/[lang]/play">) {
  const { lang } = await params;
  if (!hasLocale(lang)) notFound();
  const dict = getDictionary(lang);
  return (
    <div className="mx-auto max-w-6xl px-4 pt-10 pb-12 sm:px-6">
      <h1 className="gold-leaf text-3xl font-bold sm:text-4xl">{dict.play.title}</h1>
      <p className="mt-3 text-lg text-parchment/80">{dict.play.lead}</p>
      <p className="mt-4 rounded-md border border-gold/30 bg-gold/5 px-4 py-3 text-sm text-parchment/85">
        {dict.play.notice}{" "}
        <Link href={`/${lang}/download`} className="text-heraldic underline-offset-4 hover:underline">
          {dict.play.download}
        </Link>
      </p>
      <Suspense>
        <Client lang={lang} dict={dict} />
      </Suspense>
    </div>
  );
}

async function Client({ lang, dict }: { lang: Locale; dict: Dictionary }) {
  const account = await currentAccount();
  if (!account) {
    return (
      <div className="mt-8">
        <p className="text-parchment/85">{dict.play.signInLead}</p>
        <Link href={`/${lang}/login?next=/${lang}/play`} className={cn(buttonVariants(), "mt-5 h-11 px-6 text-base font-bold")}>
          {dict.play.signIn}
        </Link>
      </div>
    );
  }
  return (
    <GameFrame
      src={CLIENT}
      home={`/${lang}`}
      copy={{ ...dict.play, downloadHref: `/${lang}/download` }}
    />
  );
}
