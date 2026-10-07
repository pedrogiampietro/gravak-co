import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { lang as rootLang } from "next/root-params";
import { Suspense } from "react";
import { CharacterLink, CommunityNav, roleName } from "@/components/community";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { type Dictionary, getDictionary, hasLocale, type Locale } from "@/i18n/dictionaries";
import { getGuild } from "@/lib/characters";
import { date, number } from "@/lib/format";
import { professionOf } from "@/lib/game";
import { cn } from "@/lib/utils";

export async function generateMetadata({ params }: PageProps<"/[lang]/guilds/[name]">): Promise<Metadata> {
  const { name } = await params;
  return { title: decodeURIComponent(name) };
}

export default async function GuildPage({ params }: PageProps<"/[lang]/guilds/[name]">) {
  // The language is a root parameter, known before the name.
  const lang = await rootLang();
  if (!hasLocale(lang)) notFound();
  const dict = getDictionary(lang);
  return (
    <div className="mx-auto max-w-6xl px-4 pt-10 sm:px-6">
      <CommunityNav lang={lang} dict={dict} />
      <Suspense fallback={<div className="mt-10 h-96 rounded-lg border border-border bg-card/40" />}>
        <Roster lang={lang} dict={dict} params={params} />
      </Suspense>
    </div>
  );
}

async function Roster({ lang, dict, params }: { lang: Locale; dict: Dictionary; params: Promise<{ name: string }> }) {
  const name = decodeURIComponent((await params).name);
  const guild = await getGuild(name);
  const copy = dict.guilds;
  if (guild === undefined) return <p className="mt-10 text-parchment/75">{dict.ranking.offline}</p>;
  if (guild === null) {
    return (
      <div className="mt-12 grid gap-3">
        <h1 className="text-3xl font-bold text-parchment">{copy.notFound}</h1>
        <Link href={`/${lang}/guilds`} className="text-heraldic underline-offset-4 hover:underline">
          {copy.back}
        </Link>
      </div>
    );
  }
  const facts: [string, React.ReactNode][] = [
    [copy.leader, <CharacterLink key="leader" lang={lang} name={guild.leader} />],
    [copy.members, guild.members],
    [copy.averageLevel, guild.averageLevel],
    [copy.level, guild.level],
    [copy.kills, number(lang, guild.kills)],
    [copy.fund, number(lang, guild.silver)],
    [copy.founded, date(lang, guild.createdAt)],
  ];
  return (
    <div className="mt-10 grid gap-10">
      <div>
        <Link href={`/${lang}/guilds`} className="text-sm text-heraldic underline-offset-4 hover:underline">
          {copy.back}
        </Link>
        <h1 translate="no" className="gold-leaf mt-3 text-4xl font-bold sm:text-5xl">
          {guild.name}
        </h1>
      </div>
      <dl className="grid grid-cols-2 gap-x-8 gap-y-4 sm:grid-cols-4 lg:grid-cols-7">
        {facts.map(([label, value]) => (
          <div key={label} className="flex flex-col-reverse gap-0.5">
            <dt className="text-sm text-slate-blue">{label}</dt>
            <dd className="font-display text-xl font-bold text-parchment tabular-nums">{value}</dd>
          </div>
        ))}
      </dl>
      <section aria-labelledby="roster">
        <h2 id="roster" className="text-2xl font-bold text-parchment">
          {copy.roster}
        </h2>
        <div className="mt-5 overflow-x-auto rounded-lg border border-border bg-card/50">
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead>{dict.ranking.columns.name}</TableHead>
                <TableHead>{dict.ranking.columns.rank}</TableHead>
                <TableHead>{dict.ranking.columns.profession}</TableHead>
                <TableHead className="text-right">{dict.ranking.columns.level}</TableHead>
                <TableHead className="text-right">{copy.joined}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {guild.roster.map((member) => {
                const profession = professionOf(member.currentClass);
                return (
                  <TableRow key={member.name}>
                    <TableCell>
                      <span className="inline-flex items-center gap-2">
                        <span
                          aria-label={member.online ? dict.character.onlineNow : undefined}
                          className={cn("size-2 rounded-full", member.online ? "bg-emerald-400" : "bg-white/10")}
                        />
                        <CharacterLink lang={lang} name={member.name} />
                      </span>
                    </TableCell>
                    <TableCell className={member.role >= 990 ? "text-gold" : "text-parchment/80"}>{roleName(dict, member.role)}</TableCell>
                    <TableCell className="text-parchment/80">{profession ? dict.ranking.professions[profession] : "—"}</TableCell>
                    <TableCell className="text-right tabular-nums">{member.level}</TableCell>
                    <TableCell className="text-right text-parchment/75 tabular-nums">{date(lang, member.joinedAt)}</TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </div>
      </section>
    </div>
  );
}
