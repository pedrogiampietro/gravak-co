import Link from "next/link";
import type { Dictionary, Locale } from "@/i18n/dictionaries";
import { cn } from "@/lib/utils";

/** Over the pages about players: the character search. */
export function CommunityNav({ lang, dict }: { lang: Locale; dict: Dictionary }) {
  return (
    <div className="flex justify-end border-b border-border pb-4">
      <CharacterSearch lang={lang} dict={dict} />
    </div>
  );
}

/** Goes to a character's page by name. */
export function CharacterSearch({ lang, dict, defaultValue }: { lang: Locale; dict: Dictionary; defaultValue?: string }) {
  return (
    <form action={`/${lang}/characters`} method="get" role="search" className="flex gap-2">
      <label htmlFor="character-search" className="sr-only">
        {dict.community.search}
      </label>
      <input
        id="character-search"
        name="name"
        defaultValue={defaultValue}
        placeholder={dict.community.searchPlaceholder}
        autoComplete="off"
        spellCheck={false}
        maxLength={16}
        className="h-9 w-48 rounded-md border border-input bg-night/60 px-3 text-sm text-parchment placeholder:text-slate-blue focus-visible:border-gold"
      />
      <button type="submit" className="h-9 rounded-md border border-gold/40 px-3 text-sm text-gold hover:bg-gold/10">
        {dict.community.search}
      </button>
    </form>
  );
}

export function CharacterLink({ lang, name, className }: { lang: Locale; name: string; className?: string }) {
  return (
    <Link
      href={`/${lang}/characters/${encodeURIComponent(name)}`}
      translate="no"
      className={cn("font-medium text-parchment underline-offset-4 hover:text-gold hover:underline", className)}
    >
      {name}
    </Link>
  );
}

export function GuildLink({ lang, name, className }: { lang: Locale; name: string; className?: string }) {
  return (
    <Link
      href={`/${lang}/guilds/${encodeURIComponent(name)}`}
      translate="no"
      className={cn("font-medium text-parchment underline-offset-4 hover:text-gold hover:underline", className)}
    >
      {name}
    </Link>
  );
}

/** A guild role's name (the server's 1000 leader, 990 deputy, 200 member). */
export function roleName(dict: Dictionary, role: number) {
  if (role >= 1000) return dict.character.roles.leader;
  if (role >= 990) return dict.character.roles.deputy;
  return dict.character.roles.member;
}
