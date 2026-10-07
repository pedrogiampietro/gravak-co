import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { Suspense } from "react";
import { CharacterLink, CommunityNav } from "@/components/community";
import { Filters } from "@/components/filters";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { type Dictionary, getDictionary, hasLocale, type Locale } from "@/i18n/dictionaries";
import { getKillers, getLatestDeaths, type KillerPeriod, killerPeriods } from "@/lib/characters";
import { dateTime, number } from "@/lib/format";
import { type Profession, professionOf, professions } from "@/lib/game";
import { cn } from "@/lib/utils";

export async function generateMetadata({ params }: PageProps<"/[lang]/killers">): Promise<Metadata> {
  const { lang } = await params;
  return hasLocale(lang) ? { title: getDictionary(lang).killers.title } : {};
}

export default async function KillersPage({ params, searchParams }: PageProps<"/[lang]/killers">) {
  const { lang } = await params;
  if (!hasLocale(lang)) notFound();
  const dict = getDictionary(lang);
  return (
    <div className="mx-auto max-w-6xl px-4 pt-10 sm:px-6">
      <CommunityNav lang={lang} dict={dict} />
      <h1 className="gold-leaf mt-10 text-4xl font-bold sm:text-5xl">{dict.killers.title}</h1>
      <p className="mt-3 max-w-[70ch] text-parchment/75">{dict.killers.lead}</p>
      <div className="grid gap-12 lg:grid-cols-[1.4fr_1fr]">
        <Suspense fallback={<div className="mt-8 h-96 rounded-lg border border-border bg-card/40" />}>
          <Board lang={lang} dict={dict} searchParams={searchParams} />
        </Suspense>
        <Suspense fallback={<div className="mt-8 h-96" />}>
          <Latest lang={lang} dict={dict} />
        </Suspense>
      </div>
    </div>
  );
}

async function Board({
  lang,
  dict,
  searchParams,
}: {
  lang: Locale;
  dict: Dictionary;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const query = await searchParams;
  const period: KillerPeriod = (killerPeriods as readonly string[]).includes(String(query.t)) ? (query.t as KillerPeriod) : "week";
  const wanted = String(query.p);
  const profession = (professions as readonly string[]).includes(wanted) ? (wanted as Profession) : null;
  const rows = await getKillers(period, profession);
  const href = (t: KillerPeriod, p: Profession | null) => {
    const search = new URLSearchParams();
    if (t !== "week") search.set("t", t);
    if (p) search.set("p", p);
    const text = search.toString();
    return `/${lang}/killers${text ? `?${text}` : ""}`;
  };
  return (
    <div className="min-w-0">
      <Filters options={killerPeriods.map((t) => ({ href: href(t, profession), label: dict.killers.periods[t], active: t === period }))} />
      <Filters
        options={[null, ...professions].map((p) => ({
          href: href(period, p),
          label: p ? dict.ranking.professions[p] : dict.ranking.professions.all,
          active: p === profession,
        }))}
      />
      <div className="mt-6 overflow-x-auto rounded-lg border border-border bg-card/50">
        {rows === null ? (
          <p className="p-8 text-parchment/75">{dict.ranking.offline}</p>
        ) : rows.length === 0 ? (
          <p className="p-8 text-parchment/75">{dict.killers.empty}</p>
        ) : (
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead className="w-20">{dict.ranking.columns.position}</TableHead>
                <TableHead>{dict.ranking.columns.name}</TableHead>
                <TableHead>{dict.ranking.columns.profession}</TableHead>
                <TableHead className="text-right">{dict.ranking.columns.level}</TableHead>
                <TableHead className="text-right">{dict.killers.kills}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {rows.map((row, index) => {
                const rowProfession = professionOf(row.currentClass);
                return (
                  <TableRow key={row.name}>
                    <TableCell className={cn("font-display text-lg font-bold tabular-nums", index < 3 ? "text-gold" : "text-slate-blue")}>
                      {index + 1}
                    </TableCell>
                    <TableCell>
                      <CharacterLink lang={lang} name={row.name} />
                    </TableCell>
                    <TableCell className="text-parchment/80">{rowProfession ? dict.ranking.professions[rowProfession] : "—"}</TableCell>
                    <TableCell className="text-right tabular-nums">{row.level}</TableCell>
                    <TableCell className="text-right font-medium text-destructive tabular-nums">{number(lang, row.kills)}</TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        )}
      </div>
    </div>
  );
}

async function Latest({ lang, dict }: { lang: Locale; dict: Dictionary }) {
  const deaths = await getLatestDeaths();
  return (
    <section aria-labelledby="latest-deaths" className="mt-5">
      <h2 id="latest-deaths" className="text-2xl font-bold text-parchment">
        {dict.killers.latest}
      </h2>
      {!deaths || deaths.length === 0 ? (
        <p className="mt-4 text-parchment/70">{deaths ? dict.killers.latestEmpty : dict.ranking.offline}</p>
      ) : (
        <ul className="mt-4 divide-y divide-border">
          {deaths.map((death) => (
            <li key={`${death.at}-${death.victim}`} className="grid gap-0.5 py-3">
              <span className="text-xs text-slate-blue tabular-nums">{dateTime(lang, death.at)}</span>
              <span className="text-parchment/85">
                <CharacterLink lang={lang} name={death.victim} /> {dict.killers.diedTo.replace("{level}", String(death.level))}{" "}
                {death.character ? <CharacterLink lang={lang} name={death.character} /> : <span className="text-destructive">{death.name}</span>}
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
