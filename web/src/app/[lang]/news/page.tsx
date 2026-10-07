import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { categoryTone, formatDate } from "@/components/news-ticker";
import { posts } from "@/content/news";
import { getDictionary, hasLocale } from "@/i18n/dictionaries";
import { cn } from "@/lib/utils";

export async function generateMetadata({ params }: PageProps<"/[lang]/news">): Promise<Metadata> {
  const { lang } = await params;
  return hasLocale(lang) ? { title: getDictionary(lang).news.title } : {};
}

export default async function NewsPage({ params }: PageProps<"/[lang]/news">) {
  const { lang } = await params;
  if (!hasLocale(lang)) notFound();
  const dict = getDictionary(lang);
  return (
    <div className="mx-auto max-w-3xl px-4 pt-14 sm:px-6">
      <h1 className="gold-leaf text-4xl font-bold sm:text-5xl">{dict.news.title}</h1>
      <ol className="mt-10 grid gap-10">
        {posts.map((post) => (
          <li key={post.slug} className="grid gap-2 border-b border-border pb-10">
            <div className="flex items-center gap-3 text-sm">
              <time dateTime={post.date} className="text-slate-blue tabular-nums">
                {formatDate(lang, post.date, "long")}
              </time>
              <span className={cn("font-bold", categoryTone[post.category])}>{dict.news.categories[post.category]}</span>
            </div>
            <h2 className="text-2xl font-bold">
              <Link href={`/${lang}/news/${post.slug}`} className="text-parchment hover:text-gold">
                {post[lang].title}
              </Link>
            </h2>
            <p className="text-parchment/80">{post[lang].summary}</p>
          </li>
        ))}
      </ol>
    </div>
  );
}
