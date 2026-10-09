import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { Suspense } from "react";
import { type Dictionary, getDictionary, hasLocale, type Locale } from "@/i18n/dictionaries";
import { LoginForm } from "./login-form";

export async function generateMetadata({ params }: PageProps<"/[lang]/login">): Promise<Metadata> {
  const { lang } = await params;
  return hasLocale(lang) ? { title: getDictionary(lang).login.title } : {};
}

export default async function LoginPage({ params, searchParams }: PageProps<"/[lang]/login">) {
  const { lang } = await params;
  if (!hasLocale(lang)) notFound();
  const dict = getDictionary(lang);
  return (
    <div className="mx-auto max-w-md px-4 pt-14 sm:px-6">
      <h1 className="gold-leaf text-4xl font-bold sm:text-5xl">{dict.login.title}</h1>
      <p className="mt-4 text-parchment/80">{dict.login.lead}</p>
      <Suspense>
        <Form lang={lang} dict={dict} searchParams={searchParams} />
      </Suspense>
    </div>
  );
}

async function Form({ lang, dict, searchParams }: { lang: Locale; dict: Dictionary; searchParams: Promise<Record<string, string | string[] | undefined>> }) {
  const next = String((await searchParams).next ?? "");
  return <LoginForm lang={lang} copy={dict.login} next={next} registerLabel={dict.nav.register} />;
}
