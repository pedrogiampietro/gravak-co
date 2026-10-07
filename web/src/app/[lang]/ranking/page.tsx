import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { Suspense } from "react";
import { CharacterLink, CommunityNav, GuildLink } from "@/components/community";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { type Dictionary, getDictionary, hasLocale, type Locale } from "@/i18n/dictionaries";
import { nobilityNames, nobilityRank, type Profession, professionOf, professions } from "@/lib/game";
import { type Category, categories, getRanking, isCategory } from "@/lib/rankings";
import { cn } from "@/lib/utils";

export async function generateMetadata({ params }: PageProps<"/[lang]/ranking">): Promise<Metadata> {
  const { lang } = await params;
  return hasLocale(lang) ? { title: getDictionary(lang).ranking.title } : {};
}

export default async function RankingPage({ params, searchParams }: PageProps<"/[lang]/ranking">) {
  const { lang } = await params;
  if (!hasLocale(lang)) notFound();
  const dict = getDictionary(lang);
  return (
    <div className="mx-auto max-w-6xl px-4 pt-10 sm:px-6">
      <CommunityNav lang={lang} dict={dict} />
      <h1 className="gold-leaf mt-10 text-4xl font-bold sm:text-5xl">{dict.ranking.title}</h1>
      <p className="mt-3 text-parchment/75">{dict.ranking.lead}</p>
      <Suspense fallback={<div className="mt-10 h-96 rounded-lg border border-border bg-card/40" />}>
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
  const category: Category = isCategory(String(query.c)) ? (query.c as Category) : "level";
  const wanted = String(query.p);
  const profession = category !== "guilds" && (professions as readonly string[]).includes(wanted) ? (wanted as Profession) : null;
  const rows = await getRanking(category, profession);
  const href = (c: Category, p: Profession | null) => {
    const search = new URLSearchParams();
    if (c !== "level") search.set("c", c);
    if (p) search.set("p", p);
    const text = search.toString();
    return `/${lang}/ranking${text ? `?${text}` : ""}`;
  };

  return (
    <>
      <nav className="mt-10 flex flex-wrap gap-1 border-b border-border" aria-label={dict.ranking.title}>
        {categories.map((c) => (
          <Link
            key={c}
            href={href(c, c === "guilds" ? null : profession)}
            aria-current={c === category ? "page" : undefined}
            className={cn(
              "-mb-px border-b-2 px-4 py-2.5 font-display text-[0.95rem] font-bold transition-colors",
              c === category ? "border-gold text-gold" : "border-transparent text-slate-blue hover:text-parchment",
            )}
          >
            {dict.ranking.categories[c]}
          </Link>
        ))}
      </nav>

      {category !== "guilds" && (
        <div className="mt-5 flex flex-wrap items-center gap-2 text-sm">
          <span className="mr-1 text-slate-blue">{dict.ranking.filter}</span>
          {[null, ...professions].map((p) => (
            <Link
              key={p ?? "all"}
              href={href(category, p)}
              aria-current={p === profession ? "page" : undefined}
              className={cn(
                "rounded-full border px-3 py-1 transition-colors",
                p === profession ? "border-gold/60 bg-gold/10 text-gold" : "border-border text-parchment/75 hover:text-parchment",
              )}
            >
              {p ? dict.ranking.professions[p] : dict.ranking.professions.all}
            </Link>
          ))}
        </div>
      )}

      <div className="mt-6 overflow-x-auto rounded-lg border border-border bg-card/50">
        {rows === null ? (
          <p className="p-8 text-parchment/75">{dict.ranking.offline}</p>
        ) : rows.length === 0 ? (
          <p className="p-8 text-parchment/75">{dict.ranking.empty}</p>
        ) : category === "guilds" ? (
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead className="w-20">{dict.ranking.columns.position}</TableHead>
                <TableHead>{dict.ranking.columns.guild}</TableHead>
                <TableHead>{dict.ranking.columns.leader}</TableHead>
                <TableHead className="text-right">{dict.ranking.columns.members}</TableHead>
                <TableHead className="text-right">{dict.ranking.columns.value.guilds}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {rows.map((row, index) => (
                <TableRow key={row.id}>
                  <Position index={index} />
                  <TableCell>
                    <GuildLink lang={lang} name={row.name} />
                  </TableCell>
                  <TableCell>{row.leader && <CharacterLink lang={lang} name={row.leader} />}</TableCell>
                  <TableCell className="text-right tabular-nums">{row.members}</TableCell>
                  <TableCell className="text-right tabular-nums">{format(lang, row.value)}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        ) : (
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead className="w-20">{dict.ranking.columns.position}</TableHead>
                <TableHead>{dict.ranking.columns.name}</TableHead>
                <TableHead>{dict.ranking.columns.profession}</TableHead>
                <TableHead className="text-right">{dict.ranking.columns.level}</TableHead>
                {category === "nobility" && <TableHead>{dict.ranking.columns.rank}</TableHead>}
                <TableHead className="text-right">{dict.ranking.columns.value[category]}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {rows.map((row, index) => {
                const rowProfession = professionOf(row.currentClass);
                // The title follows the place among all donors, so only the
                // unfiltered list can name it.
                const rank = category === "nobility" && !profession ? nobilityRank(index + 1, row.value) : null;
                return (
                  <TableRow key={row.id}>
                    <Position index={index} />
                    <TableCell>
                      <CharacterLink lang={lang} name={row.name} />
                    </TableCell>
                    <TableCell className="text-parchment/80">{rowProfession ? dict.ranking.professions[rowProfession] : "—"}</TableCell>
                    <TableCell className="text-right tabular-nums">{row.level}</TableCell>
                    {category === "nobility" && (
                      <TableCell className="text-gold">{rank ? nobilityNames[rank] : "—"}</TableCell>
                    )}
                    <TableCell className="text-right tabular-nums">{format(lang, row.value)}</TableCell>
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

function Position({ index }: { index: number }) {
  return (
    <TableCell className={cn("font-display text-lg font-bold tabular-nums", index < 3 ? "text-gold" : "text-slate-blue")}>
      {index + 1}
    </TableCell>
  );
}

function format(lang: Locale, value: number) {
  return new Intl.NumberFormat(lang === "pt" ? "pt-BR" : "en-US").format(value);
}
