import type { Metadata } from "next";
import Image from "next/image";
import { notFound } from "next/navigation";
import { getDictionary, hasLocale } from "@/i18n/dictionaries";
import { RegisterForm } from "./register-form";

export async function generateMetadata({ params }: PageProps<"/[lang]/register">): Promise<Metadata> {
  const { lang } = await params;
  return hasLocale(lang) ? { title: getDictionary(lang).register.title } : {};
}

export default async function RegisterPage({ params }: PageProps<"/[lang]/register">) {
  const { lang } = await params;
  if (!hasLocale(lang)) notFound();
  const dict = getDictionary(lang);
  return (
    <div className="mx-auto grid max-w-6xl gap-12 px-4 pt-14 sm:px-6 lg:grid-cols-[1fr_1.1fr] lg:items-start">
      <div className="max-w-md">
        <h1 className="gold-leaf text-4xl font-bold sm:text-5xl">{dict.register.title}</h1>
        <p className="mt-4 text-parchment/80">{dict.register.lead}</p>
        <RegisterForm lang={lang} copy={dict.register} />
      </div>
      <div className="relative hidden aspect-[7/9] max-h-[640px] w-full max-w-full lg:block" aria-hidden="true">
        <div className="absolute inset-x-[22%] bottom-[5%] h-[5%] rounded-[50%] bg-black/55 blur-md" />
        <Image src="/art/warrior.png" alt="" fill sizes="45vw" className="object-contain object-bottom" />
      </div>
    </div>
  );
}
