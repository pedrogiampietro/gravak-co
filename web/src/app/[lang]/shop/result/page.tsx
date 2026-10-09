import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { Suspense } from "react";
import { buttonVariants } from "@/components/ui/button";
import { type Dictionary, getDictionary, hasLocale, type Locale } from "@/i18n/dictionaries";
import { cn } from "@/lib/utils";

export async function generateMetadata({ params }: PageProps<"/[lang]/shop/result">): Promise<Metadata> {
  const { lang } = await params;
  return hasLocale(lang) ? { title: getDictionary(lang).shop.title } : {};
}

// Where Checkout Pro sends the buyer back (back_urls), with Mercado Pago's
// `status`. The CPs come from the notification, not from this page.
export default async function ShopResultPage({ params, searchParams }: PageProps<"/[lang]/shop/result">) {
  const { lang } = await params;
  if (!hasLocale(lang)) notFound();
  const dict = getDictionary(lang);
  return (
    <div className="mx-auto max-w-2xl px-4 pt-14 sm:px-6">
      <Suspense>
        <Result lang={lang} dict={dict} searchParams={searchParams} />
      </Suspense>
    </div>
  );
}

async function Result({ lang, dict, searchParams }: { lang: Locale; dict: Dictionary; searchParams: Promise<Record<string, string | string[] | undefined>> }) {
  const status = String((await searchParams).status ?? "");
  const copy = dict.shop.result;
  const outcome = status === "approved" ? copy.approved : status === "pending" || status === "in_process" ? copy.pending : copy.failure;
  return (
    <>
      <h1 className="gold-leaf text-4xl font-bold sm:text-5xl">{outcome.title}</h1>
      <p className="mt-4 text-lg text-parchment/85">{outcome.text}</p>
      <Link href={`/${lang}/shop`} className={cn(buttonVariants(), "mt-8 h-11 px-6 text-base font-bold")}>
        {copy.back}
      </Link>
    </>
  );
}
