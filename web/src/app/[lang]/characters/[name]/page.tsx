import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { lang as rootLang } from "next/root-params";
import { Suspense } from "react";
import { CharacterLink, CharacterSearch, CommunityNav, GuildLink, roleName } from "@/components/community";
import { EquipmentWindow } from "@/components/equipment-window";
import { type Dictionary, getDictionary, hasLocale, type Locale } from "@/i18n/dictionaries";
import { type Character, getCharacter, type Worn } from "@/lib/characters";
import { ago, date, dateTime, number } from "@/lib/format";
import { nobilityNames, professionOf } from "@/lib/game";
import { cn } from "@/lib/utils";

export async function generateMetadata({ params }: PageProps<"/[lang]/characters/[name]">): Promise<Metadata> {
  const { name } = await params;
  return { title: decodeURIComponent(name) };
}

export default async function CharacterPage({ params }: PageProps<"/[lang]/characters/[name]">) {
  // The language is a root parameter, known before the name.
  const lang = await rootLang();
  if (!hasLocale(lang)) notFound();
  const dict = getDictionary(lang);
  return (
    <div className="mx-auto max-w-6xl px-4 pt-10 sm:px-6">
      <CommunityNav lang={lang} dict={dict} />
      <Suspense fallback={<div className="mt-10 h-96 rounded-lg border border-border bg-card/40" />}>
        <Profile lang={lang} dict={dict} params={params} />
      </Suspense>
    </div>
  );
}

async function Profile({ lang, dict, params }: { lang: Locale; dict: Dictionary; params: Promise<{ name: string }> }) {
  const name = decodeURIComponent((await params).name);
  const character = await getCharacter(name);
  if (character === undefined) return <p className="mt-10 text-parchment/75">{dict.character.offline}</p>;
  if (character === null) {
    return (
      <div className="mt-12 grid max-w-lg gap-4">
        <h1 className="text-3xl font-bold text-parchment">{dict.character.notFound}</h1>
        <p className="text-parchment/75">{dict.character.notFoundHint}</p>
        <CharacterSearch lang={lang} dict={dict} defaultValue={name} />
      </div>
    );
  }
  const copy = dict.character;
  const profession = professionOf(character.currentClass);

  return (
    <div className="mt-10 grid gap-12">
      {/* Who: the name and whether they are in the world. */}
      <header className="flex flex-col gap-6 sm:flex-row sm:items-end">
        <div className="grid gap-2">
          <h1 translate="no" className="gold-leaf text-4xl font-bold sm:text-5xl">
            {character.name}
          </h1>
          <p className="text-lg text-parchment/85">
            {profession ? dict.ranking.professions[profession] : "—"} · {copy.level} {character.level}
            {character.rebirths > 0 && ` · ${character.rebirths} ${copy.rebirths.toLowerCase()}`}
          </p>
          <Presence lang={lang} dict={dict} character={character} />
        </div>
      </header>

      <div className="grid gap-10 md:grid-cols-[minmax(0,396px)_1fr] md:items-start">
        {/* The game's own Character window, its Equipment tab. */}
        <section aria-labelledby="equipment" className="grid justify-items-center gap-3 md:justify-items-start">
          <h2 id="equipment" className="sr-only">
            {copy.equipment}
          </h2>
          <EquipmentWindow
            equipment={character.equipment}
            level={character.level}
            profession={profession}
            slotNames={copy.slots}
            label={`${copy.equipment}: ${character.name}`}
          />
        </section>

        <div className="grid min-w-0 gap-8">
          <section aria-labelledby="facts">
            <h2 id="facts" className="sr-only">
              {character.name}
            </h2>
            <dl className="grid divide-y divide-border rounded-lg border border-border bg-card/50">
              <Fact label={copy.profession}>{profession ? dict.ranking.professions[profession] : "—"}</Fact>
              <Fact label={copy.level}>{character.level}</Fact>
              <Fact label={copy.rebirths}>{character.rebirths}</Fact>
              <Fact label={copy.guild}>
                {character.guild ? (
                  <>
                    <GuildLink lang={lang} name={character.guild.name} />{" "}
                    <span className="text-slate-blue">({roleName(dict, character.guild.role)})</span>
                  </>
                ) : (
                  "—"
                )}
              </Fact>
              <Fact label={copy.spouse}>{character.spouse && character.spouse !== "None" ? <CharacterLink lang={lang} name={character.spouse} /> : copy.single}</Fact>
              <Fact label={copy.nobility}>
                {character.nobility ? (
                  <span className="text-gold">
                    {nobilityNames[character.nobility]}
                    {character.nobilityPlace && (
                      <span className="text-slate-blue">
                        {" "}
                        · {character.nobilityPlace}º {copy.place}
                      </span>
                    )}
                  </span>
                ) : (
                  copy.commoner
                )}
              </Fact>
              <Fact label={copy.pk}>{number(lang, character.pkPoints)}</Fact>
              <Fact label={copy.bestKo}>{number(lang, character.bestKo)}</Fact>
              {character.createdAt > 0 && <Fact label={copy.created}>{date(lang, character.createdAt)}</Fact>}
            </dl>
          </section>
          <section aria-labelledby="worn">
            <h2 id="worn" className="text-xl font-bold text-parchment">
              {copy.equipment}
            </h2>
            <ul className="mt-3 grid gap-1.5">
              {copy.slots.map((slot, index) => {
                const worn = character.equipment.find((item) => item.position === index + 1);
                if (!worn) return null;
                return (
                  <li key={slot} className="grid grid-cols-[7.5rem_1fr] gap-3 text-[0.95rem]">
                    <span className="text-slate-blue">{slot}</span>
                    <span className={cn("min-w-0 truncate font-medium", qualityTone[worn.quality])}>
                      {worn.quality !== "Normal" && `${worn.quality} `}
                      {worn.name}
                      {worn.plus > 0 && <span className="text-gold"> +{worn.plus}</span>}
                    </span>
                  </li>
                );
              })}
              {character.equipment.length === 0 && <li className="text-parchment/60">{copy.emptySlot}</li>}
            </ul>
          </section>
        </div>
      </div>

      <section aria-labelledby="history">
        <h2 id="history" className="text-2xl font-bold text-parchment">
          {copy.history}
        </h2>
        {character.history.length === 0 ? (
          <p className="mt-4 text-parchment/70">{copy.noHistory}</p>
        ) : (
          <ol className="mt-5 grid gap-2">
            {character.history.map((day) => {
              const most = Math.max(...character.history.map((entry) => entry.gained), 1);
              return (
                <li key={day.day} className="grid grid-cols-[6.5rem_1fr_auto] items-center gap-4 text-sm tabular-nums">
                  <time dateTime={day.day} className="text-slate-blue">
                    {new Intl.DateTimeFormat(lang === "pt" ? "pt-BR" : "en-US", { day: "2-digit", month: "short", timeZone: "UTC" }).format(
                      new Date(`${day.day}T00:00:00Z`),
                    )}
                  </time>
                  <span className="h-2 overflow-hidden rounded-full bg-lapis">
                    <span className="block h-full rounded-full bg-gold" style={{ width: `${(day.gained / most) * 100}%` }} />
                  </span>
                  <span className="w-48 text-right text-parchment/85">
                    +{number(lang, day.gained)} · {copy.level} {day.level}
                  </span>
                </li>
              );
            })}
          </ol>
        )}
      </section>

      <div className="grid gap-12 md:grid-cols-2">
        <section aria-labelledby="deaths">
          <h2 id="deaths" className="text-2xl font-bold text-parchment">
            {copy.deaths}
          </h2>
          {character.deaths.length === 0 ? (
            <p className="mt-4 text-parchment/70">{copy.noDeaths}</p>
          ) : (
            <ul className="mt-4 divide-y divide-border">
              {character.deaths.map((death) => (
                <li key={death.at} className="grid gap-0.5 py-3">
                  <span className="text-xs text-slate-blue tabular-nums">{dateTime(lang, death.at)}</span>
                  <span className="text-parchment/85">
                    {copy.killedBy.replace("{level}", String(death.level))}{" "}
                    {death.character ? <CharacterLink lang={lang} name={death.character} /> : <span className="text-destructive">{death.name}</span>}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </section>
        <section aria-labelledby="kills">
          <h2 id="kills" className="text-2xl font-bold text-parchment">
            {copy.kills}
          </h2>
          {character.kills.length === 0 ? (
            <p className="mt-4 text-parchment/70">{copy.noKills}</p>
          ) : (
            <ul className="mt-4 divide-y divide-border">
              {character.kills.map((kill) => (
                <li key={kill.at} className="grid gap-0.5 py-3">
                  <span className="text-xs text-slate-blue tabular-nums">{dateTime(lang, kill.at)}</span>
                  <span className="text-parchment/85">
                    {copy.killed} <CharacterLink lang={lang} name={kill.name} />{" "}
                    {kill.otherLevel !== null && (
                      <span className="text-slate-blue">
                        ({copy.atLevel} {kill.otherLevel})
                      </span>
                    )}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>
    </div>
  );
}

function Presence({ lang, dict, character }: { lang: Locale; dict: Dictionary; character: Character }) {
  if (character.online) {
    return (
      <span className="inline-flex items-center gap-2 text-sm text-emerald-300">
        <span aria-hidden="true" className="size-2 rounded-full bg-emerald-400 shadow-[0_0_8px_rgb(52_211_153/0.8)]" />
        {dict.character.onlineNow}
      </span>
    );
  }
  return (
    <span className="text-sm text-slate-blue">
      {character.lastSeen > 0 ? `${dict.character.lastSeen} ${ago(lang, character.lastSeen)}` : dict.character.neverSeen}
    </span>
  );
}

function Fact({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[9.5rem_1fr] gap-4 px-4 py-2.5">
      <dt className="text-slate-blue">{label}</dt>
      <dd className="min-w-0 text-parchment">{children}</dd>
    </div>
  );
}

const qualityTone: Record<Worn["quality"], string> = {
  Super: "text-gold",
  Elite: "text-violet-300",
  Unique: "text-sky-300",
  Refined: "text-emerald-300",
  Normal: "text-parchment",
};


