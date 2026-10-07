import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { Suspense } from "react";
import { CharacterLink, CommunityNav } from "@/components/community";
import { Filters } from "@/components/filters";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { type Dictionary, getDictionary, hasLocale, type Locale } from "@/i18n/dictionaries";
import { getPowergamers } from "@/lib/characters";
import { number } from "@/lib/format";
import { type Profession, professionOf, professions } from "@/lib/game";
import { cn } from "@/lib/utils";

const PERIODS = [1, 2, 3, 7];

export async function generateMetadata({ params }: PageProps<"/[lang]/powergamers">): Promise<Metadata> {
  const { lang } = await params;
  return hasLocale(lang) ? { title: getDictionary(lang).powergamers.title } : {};
}

export default async function PowergamersPage({ params, searchParams }: PageProps<"/[lang]/powergamers">) {
  const { lang } = await params;
  if (!hasLocale(lang)) notFound();
  const dict = getDictionary(lang);
  return (
    <div className="mx-auto max-w-6xl px-4 pt-10 sm:px-6">
      <CommunityNav lang={lang} dict={dict} />
      <h1 className="gold-leaf mt-10 text-4xl font-bold sm:text-5xl">{dict.powergamers.title}</h1>
      <p className="mt-3 max-w-[70ch] text-parchment/75">{dict.powergamers.lead}</p>
      <Suspense fallback={<div className="mt-8 h-96 rounded-lg border border-border bg-card/40" />}>
        <Board lang={lang} dict={dict} searchParams={searchParams} />
      </Suspense>
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
  const days = PERIODS.includes(Number(query.d)) ? Number(query.d) : 1;
  const wanted = String(query.p);
  const profession = (professions as readonly string[]).includes(wanted) ? (wanted as Profession) : null;
  const rows = await getPowergamers(days, profession);
  const href = (d: number, p: Profession | null) => {
    const search = new URLSearchParams();
    if (d !== 1) search.set("d", String(d));
    if (p) search.set("p", p);
    const text = search.toString();
    return `/${lang}/powergamers${text ? `?${text}` : ""}`;
  };
  return (
    <>
      <Filters
        options={PERIODS.map((d, index) => ({ href: href(d, profession), label: dict.powergamers.periods[index], active: d === days }))}
      />
      <Filters
        options={[null, ...professions].map((p) => ({
          href: href(days, p),
          label: p ? dict.ranking.professions[p] : dict.ranking.professions.all,
          active: p === profession,
        }))}
      />
      <div className="mt-6 overflow-x-auto rounded-lg border border-border bg-card/50">
        {rows === null ? (
          <p className="p-8 text-parchment/75">{dict.ranking.offline}</p>
        ) : rows.length === 0 ? (
          <p className="p-8 text-parchment/75">{dict.powergamers.empty}</p>
        ) : (
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead className="w-20">{dict.ranking.columns.position}</TableHead>
                <TableHead>{dict.ranking.columns.name}</TableHead>
                <TableHead>{dict.ranking.columns.profession}</TableHead>
                <TableHead className="text-right">{dict.ranking.columns.level}</TableHead>
                <TableHead className="text-right">{dict.powergamers.gained}</TableHead>
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
                    <TableCell className="text-right font-medium text-emerald-300 tabular-nums">+{number(lang, row.gained)}</TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        )}
      </div>
    </>
  );
}
