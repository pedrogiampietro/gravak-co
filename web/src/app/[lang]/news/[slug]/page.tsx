import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { Suspense } from "react";
import { categoryTone, formatDate } from "@/components/news-ticker";
import { findPost, posts } from "@/content/news";
import { getDictionary, hasLocale, type Locale, locales } from "@/i18n/dictionaries";
import { cn } from "@/lib/utils";

export function generateStaticParams() {
  return locales.flatMap((lang) => posts.map((post) => ({ lang, slug: post.slug })));
}

export async function generateMetadata({ params }: PageProps<"/[lang]/news/[slug]">): Promise<Metadata> {
  const { lang, slug } = await params;
  const post = findPost(slug);
  if (!hasLocale(lang) || !post) return {};
  return { title: post[lang].title, description: post[lang].summary };
}

export default function PostPage({ params }: PageProps<"/[lang]/news/[slug]">) {
  return (
    <Suspense fallback={<div className="mx-auto h-96 max-w-3xl" />}>
      <Post params={params} />
    </Suspense>
  );
}

async function Post({ params }: { params: Promise<{ lang: string; slug: string }> }) {
  const { lang, slug } = await params;
  const post = findPost(slug);
  if (!hasLocale(lang) || !post) notFound();
  const dict = getDictionary(lang as Locale);
  const text = post[lang];
  return (
    <article className="mx-auto max-w-3xl px-4 pt-14 sm:px-6">
      <Link href={`/${lang}/news`} className="text-sm text-heraldic underline-offset-4 hover:underline">
        {dict.news.back}
      </Link>
      <div className="mt-6 flex items-center gap-3 text-sm">
        <time dateTime={post.date} className="text-slate-blue tabular-nums">
          {formatDate(lang, post.date, "long")}
        </time>
        <span className={cn("font-bold", categoryTone[post.category])}>{dict.news.categories[post.category]}</span>
      </div>
      <h1 className="gold-leaf mt-3 text-4xl font-bold sm:text-5xl">{text.title}</h1>
      <p className="mt-5 text-lg text-parchment/85">{text.summary}</p>
      <div className="mt-8 grid max-w-[65ch] gap-4 text-parchment/85">
        {text.body.map((paragraph) => (
          <p key={paragraph}>{paragraph}</p>
        ))}
      </div>
    </article>
  );
}
