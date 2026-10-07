import { notFound, redirect } from "next/navigation";
import { Suspense } from "react";
import { CharacterSearch, CommunityNav } from "@/components/community";
import { type Dictionary, getDictionary, hasLocale, type Locale } from "@/i18n/dictionaries";

// `/characters?name=X`, the search form's target, goes to X's page.
export default async function CharactersPage({ params, searchParams }: PageProps<"/[lang]/characters">) {
  const { lang } = await params;
  if (!hasLocale(lang)) notFound();
  const dict = getDictionary(lang);
  return (
    <div className="mx-auto max-w-6xl px-4 pt-10 sm:px-6">
      <CommunityNav lang={lang} dict={dict} />
      <Suspense>
        <Search lang={lang} dict={dict} searchParams={searchParams} />
      </Suspense>
    </div>
  );
}

async function Search({
  lang,
  dict,
  searchParams,
}: {
  lang: Locale;
  dict: Dictionary;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const name = String((await searchParams).name ?? "").trim();
  if (name) redirect(`/${lang}/characters/${encodeURIComponent(name)}`);
  return (
    <div className="mt-12 grid max-w-lg gap-4">
      <h1 className="gold-leaf text-4xl font-bold">{dict.community.search}</h1>
      <CharacterSearch lang={lang} dict={dict} />
    </div>
  );
}
