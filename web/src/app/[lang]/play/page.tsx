import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { getDictionary, hasLocale } from "@/i18n/dictionaries";

// The client compiled to wasm (docs/web-spike.md), copied into public/play at
// deploy by client/tools/build-web.ps1. A frame of its own: the client takes
// the whole page it runs in.
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
      <iframe
        src={CLIENT}
        title="Gravak"
        className="mt-6 aspect-[4/3] w-full rounded-md border border-border bg-black"
        allow="fullscreen; autoplay; clipboard-read; clipboard-write"
        allowFullScreen
      />
    </div>
  );
}
