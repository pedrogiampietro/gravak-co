import Image from "next/image";
import Link from "next/link";
import { notFound } from "next/navigation";
import { Suspense } from "react";
import { CharacterLink } from "@/components/community";
import { NewsTicker } from "@/components/news-ticker";
import { buttonVariants } from "@/components/ui/button";
import { WarCountdown } from "@/components/war-countdown";
import { type Dictionary, getDictionary, hasLocale, type Locale } from "@/i18n/dictionaries";
import { professionOf, professions } from "@/lib/game";
import { getRanking } from "@/lib/rankings";
import { cn } from "@/lib/utils";

const DISCORD = process.env.NEXT_PUBLIC_DISCORD_URL;
// The server's clock for its events (the game server's `GUILD_WAR_UTC_OFFSET`).
const SERVER_UTC_OFFSET = Number(process.env.GUILD_WAR_UTC_OFFSET ?? -3);

const places = [
  { image: "/art/bird-island.jpg", name: "Bird Island" },
  { image: "/art/phoenix-castle.jpg", name: "Phoenix Castle" },
  { image: "/art/desert-city.jpg", name: "Desert City" },
  { image: "/art/ape-city.jpg", name: "Ape City" },
];

export default async function Home({ params }: PageProps<"/[lang]">) {
  const { lang } = await params;
  if (!hasLocale(lang)) notFound();
  const dict = getDictionary(lang);

  return (
    <>
      {/* Twin City, as our client draws it, under the thesis. */}
      <section className="relative isolate overflow-hidden border-b border-white/5">
        <Image
          src="/art/twin-city.jpg"
          alt=""
          fill
          priority
          sizes="100vw"
          className="-z-20 object-cover object-[30%_40%] brightness-125 saturate-110"
        />
        <div className="absolute inset-0 -z-10 bg-[linear-gradient(90deg,rgb(12_23_41/0.95)_0%,rgb(12_23_41/0.7)_38%,rgb(12_23_41/0.05)_72%,rgb(12_23_41/0.25)_100%)]" />
        <div className="absolute inset-x-0 bottom-0 -z-10 h-40 bg-gradient-to-t from-night to-transparent" />
        <div className="mx-auto grid max-w-6xl gap-12 px-4 pt-20 pb-24 sm:px-6 lg:grid-cols-[1.25fr_1fr] lg:items-end lg:pt-32 lg:pb-32">
          <div className="rise max-w-xl">
            <h1 className="gold-leaf font-display text-[clamp(2.6rem,6vw,4.6rem)] font-bold">{dict.hero.title}</h1>
            <p className="mt-6 max-w-[60ch] text-lg text-parchment/85">{dict.hero.lead}</p>
            <div className="mt-9 flex flex-wrap gap-3">
              <Link href={`/${lang}/download`} className={cn(buttonVariants(), "h-12 px-6 text-base font-bold")}>
                {dict.hero.download}
              </Link>
              <Link
                href={`/${lang}/register`}
                className={cn(buttonVariants({ variant: "outline" }), "h-12 border-gold/40 bg-night/40 px-6 text-base")}
              >
                {dict.hero.register}
              </Link>
            </div>
          </div>
          <div className="rise-late">
            <Suspense fallback={<LeadersFrame dict={dict} lang={lang} />}>
              <Leaders dict={dict} lang={lang} />
            </Suspense>
          </div>
        </div>
      </section>

      {/* The facts players look for first, as the server has them. */}
      <section aria-label={dict.specs.label} className="border-b border-white/5 bg-lapis/30">
        <dl className="mx-auto grid max-w-6xl grid-cols-3 gap-y-6 px-4 py-7 sm:px-6 md:grid-cols-6">
          {dict.specs.items.map((item) => (
            <div key={item.name} className="flex flex-col-reverse items-start gap-0.5">
              <dt className="text-sm text-slate-blue">{item.name}</dt>
              <dd className="font-display text-3xl font-bold text-gold tabular-nums">{item.value}</dd>
            </div>
          ))}
        </dl>
      </section>

      <div className="mx-auto max-w-6xl px-4 pt-14 sm:px-6">
        <NewsTicker lang={lang} dict={dict} />
      </div>

      {/* The professions, rendered by the client's software rasteriser. */}
      <section className="mx-auto max-w-6xl px-4 pt-24 sm:px-6">
        <h2 className="text-3xl font-bold text-parchment sm:text-4xl">{dict.classes.title}</h2>
        <p className="mt-3 max-w-[62ch] text-parchment/75">{dict.classes.lead}</p>
        <ul className="mt-12 grid grid-cols-2 gap-x-6 gap-y-12 lg:grid-cols-4">
          {professions.map((profession) => {
            const info = dict.classes[profession];
            return (
              <li key={profession} className="flex flex-col">
                <div className="relative aspect-[7/9] w-full max-w-full">
                  <div className="absolute inset-x-[18%] bottom-[5%] h-[6%] rounded-[50%] bg-black/55 blur-md" />
                  <Image
                    src={`/art/${profession}.png`}
                    alt={info.name}
                    fill
                    sizes="(min-width: 1024px) 25vw, 50vw"
                    className="object-contain object-bottom"
                  />
                </div>
                <h3 className="mt-2 text-2xl font-bold text-gold">{info.name}</h3>
                <p className="mt-2 text-[0.98rem] text-parchment/75">{info.text}</p>
              </li>
            );
          })}
        </ul>
      </section>

      <section className="mx-auto max-w-6xl px-4 pt-24 sm:px-6">
        <h2 className="text-3xl font-bold text-parchment sm:text-4xl">{dict.systems.title}</h2>
        <dl className="mt-10 grid gap-x-12 gap-y-8 border-t border-border pt-10 md:grid-cols-2">
          {dict.systems.items.map((item) => (
            <div key={item.name} className="grid gap-1.5 sm:grid-cols-[11rem_1fr] sm:gap-6">
              <dt className="font-display text-lg font-bold text-gold">{item.name}</dt>
              <dd className="text-parchment/80">{item.text}</dd>
            </div>
          ))}
        </dl>
      </section>

      <section className="mx-auto max-w-6xl px-4 pt-24 sm:px-6">
        <div className="flex flex-wrap items-end justify-between gap-4">
          <h2 className="text-3xl font-bold text-parchment sm:text-4xl">{dict.agenda.title}</h2>
          <WarCountdown offset={SERVER_UTC_OFFSET} labels={dict.war} />
        </div>
        <ul className="mt-10 grid gap-px overflow-hidden rounded-lg border border-border bg-border md:grid-cols-3">
          {dict.agenda.items.map((item) => (
            <li key={item.name} className="flex flex-col gap-2 bg-night p-6">
              <span className="text-sm font-medium text-heraldic">{item.when}</span>
              <h3 className="text-xl font-bold text-gold">{item.name}</h3>
              <p className="text-[0.98rem] text-parchment/80">{item.text}</p>
            </li>
          ))}
        </ul>
        <p className="mt-3 text-sm text-slate-blue">{dict.agenda.timezone}</p>
      </section>

      {/* The world beyond the city, side by side; scrolls on phones. */}
      <section className="pt-24" aria-label="Twin City, Bird Island, Phoenix Castle, Desert City, Ape City">
        <ul className="mx-auto flex max-w-6xl snap-x gap-4 overflow-x-auto px-4 pb-2 sm:px-6">
          {places.map((place) => (
            <li key={place.name} className="relative aspect-[8/5] w-[78%] max-w-full shrink-0 snap-start overflow-hidden rounded-md sm:w-[46%] lg:w-[calc(25%-0.75rem)]">
              <Image src={place.image} alt={place.name} fill sizes="(min-width: 1024px) 25vw, 80vw" className="object-cover" />
              <span className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-night/95 to-transparent px-3 pt-10 pb-2.5 font-display text-sm font-bold text-parchment">
                {place.name}
              </span>
            </li>
          ))}
        </ul>
      </section>

      <section className="mx-auto max-w-6xl px-4 pt-24 sm:px-6">
        <div className="flex flex-col gap-6 border-y border-gold/25 py-12 md:flex-row md:items-center md:justify-between">
          <div>
            <h2 className="text-3xl font-bold text-parchment">{dict.cta.title}</h2>
            <p className="mt-2 text-parchment/75">{dict.cta.text}</p>
          </div>
          <div className="flex flex-wrap gap-3">
            <Link href={`/${lang}/download`} className={cn(buttonVariants(), "h-12 px-6 text-base font-bold")}>
              {dict.hero.download}
            </Link>
            <Link href={`/${lang}/register`} className={cn(buttonVariants({ variant: "outline" }), "h-12 border-gold/40 px-6 text-base")}>
              {dict.hero.register}
            </Link>
            {DISCORD && (
              <a href={DISCORD} className={cn(buttonVariants({ variant: "ghost" }), "h-12 px-5 text-base text-heraldic")}>
                {dict.cta.discord}
              </a>
            )}
          </div>
        </div>
      </section>
    </>
  );
}

function LeadersFrame({ dict, lang, children }: { dict: Dictionary; lang: Locale; children?: React.ReactNode }) {
  return (
    <div className="rounded-lg border border-gold/25 bg-night/75 p-5 shadow-[0_20px_60px_rgb(0_0_0/0.45)] backdrop-blur-sm">
      <h2 className="font-display text-base font-bold text-gold">{dict.hero.leaders}</h2>
      <div className="mt-3 min-h-[13.5rem]">{children}</div>
      <Link href={`/${lang}/ranking`} className="mt-3 inline-block text-sm text-heraldic underline-offset-4 hover:underline">
        {dict.hero.seeRanking}
      </Link>
    </div>
  );
}

async function Leaders({ dict, lang }: { dict: Dictionary; lang: Locale }) {
  const rows = (await getRanking("level", null)) ?? [];
  const top = rows.slice(0, 5);
  return (
    <LeadersFrame dict={dict} lang={lang}>
      {top.length === 0 ? (
        <p className="pt-2 text-parchment/70">{dict.hero.leadersEmpty}</p>
      ) : (
        <ol className="divide-y divide-white/5">
          {top.map((row, index) => {
            const profession = professionOf(row.currentClass);
            return (
              <li key={row.id} className="flex items-center gap-3 py-2.5 tabular-nums">
                <span className={cn("w-6 font-display text-lg font-bold", index === 0 ? "text-gold" : "text-slate-blue")}>
                  {index + 1}
                </span>
                <CharacterLink lang={lang} name={row.name} className="min-w-0 flex-1 truncate" />
                <span className="text-sm text-slate-blue">{profession ? dict.classes[profession].name : ""}</span>
                <span className="w-16 text-right text-sm text-parchment/85">
                  {dict.hero.level} {row.level}
                </span>
              </li>
            );
          })}
        </ol>
      )}
    </LeadersFrame>
  );
}
