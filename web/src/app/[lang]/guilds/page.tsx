import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { Suspense } from "react";
import { CharacterLink, CommunityNav, GuildLink } from "@/components/community";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { type Dictionary, getDictionary, hasLocale, type Locale } from "@/i18n/dictionaries";
import { getGuilds } from "@/lib/characters";
import { date, number } from "@/lib/format";

export async function generateMetadata({ params }: PageProps<"/[lang]/guilds">): Promise<Metadata> {
  const { lang } = await params;
  return hasLocale(lang) ? { title: getDictionary(lang).guilds.title } : {};
}

export default async function GuildsPage({ params }: PageProps<"/[lang]/guilds">) {
  const { lang } = await params;
  if (!hasLocale(lang)) notFound();
  const dict = getDictionary(lang);
  return (
    <div className="mx-auto max-w-6xl px-4 pt-10 sm:px-6">
      <CommunityNav lang={lang} dict={dict} />
      <h1 className="gold-leaf mt-10 text-4xl font-bold sm:text-5xl">{dict.guilds.title}</h1>
      <p className="mt-3 text-parchment/75">{dict.guilds.lead}</p>
      <Suspense fallback={<div className="mt-8 h-96 rounded-lg border border-border bg-card/40" />}>
        <List lang={lang} dict={dict} />
      </Suspense>
    </div>
  );
}

async function List({ lang, dict }: { lang: Locale; dict: Dictionary }) {
  const guilds = await getGuilds();
  const copy = dict.guilds;
  return (
    <div className="mt-8 overflow-x-auto rounded-lg border border-border bg-card/50">
      {guilds === null ? (
        <p className="p-8 text-parchment/75">{dict.ranking.offline}</p>
      ) : guilds.length === 0 ? (
        <p className="p-8 text-parchment/75">{copy.empty}</p>
      ) : (
        <Table>
          <TableHeader>
            <TableRow className="hover:bg-transparent">
              <TableHead>{copy.guild}</TableHead>
              <TableHead>{copy.leader}</TableHead>
              <TableHead className="text-right">{copy.members}</TableHead>
              <TableHead className="text-right">{copy.averageLevel}</TableHead>
              <TableHead className="text-right">{copy.kills}</TableHead>
              <TableHead className="text-right">{copy.fund}</TableHead>
              <TableHead className="text-right">{copy.founded}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {guilds.map((guild) => (
              <TableRow key={guild.name}>
                <TableCell>
                  <GuildLink lang={lang} name={guild.name} className="font-display font-bold" />
                </TableCell>
                <TableCell>
                  <CharacterLink lang={lang} name={guild.leader} />
                </TableCell>
                <TableCell className="text-right tabular-nums">{guild.members}</TableCell>
                <TableCell className="text-right tabular-nums">{guild.averageLevel}</TableCell>
                <TableCell className="text-right tabular-nums">{number(lang, guild.kills)}</TableCell>
                <TableCell className="text-right tabular-nums">{number(lang, guild.silver)}</TableCell>
                <TableCell className="text-right text-parchment/75 tabular-nums">{date(lang, guild.createdAt)}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </div>
  );
}
