import type { Metadata } from "next";
import Image from "next/image";
import Link from "next/link";
import { notFound } from "next/navigation";
import { buttonVariants } from "@/components/ui/button";
import { getDictionary, hasLocale } from "@/i18n/dictionaries";
import { cn } from "@/lib/utils";

// Where the installer is published (package-client.bat makes GravakSetup.exe).
const INSTALLER = process.env.NEXT_PUBLIC_DOWNLOAD_URL ?? "/downloads/GravakSetup.exe";

export async function generateMetadata({ params }: PageProps<"/[lang]/download">): Promise<Metadata> {
  const { lang } = await params;
  return hasLocale(lang) ? { title: getDictionary(lang).download.title } : {};
}

export default async function DownloadPage({ params }: PageProps<"/[lang]/download">) {
  const { lang } = await params;
  if (!hasLocale(lang)) notFound();
  const dict = getDictionary(lang);
  return (
    <div className="mx-auto max-w-6xl px-4 pt-14 sm:px-6">
      <div className="grid gap-12 lg:grid-cols-[1.2fr_1fr] lg:items-end">
        <div>
          <h1 className="gold-leaf text-4xl font-bold sm:text-5xl">{dict.download.title}</h1>
          <p className="mt-4 max-w-[58ch] text-lg text-parchment/80">{dict.download.lead}</p>
          <a href={INSTALLER} className={cn(buttonVariants(), "mt-8 h-12 px-7 text-base font-bold")}>
            {dict.download.button}
          </a>
          <p className="mt-3 text-sm text-slate-blue">GravakSetup.exe · {dict.download.size}</p>
        </div>
        <div className="relative hidden aspect-[7/9] max-h-[460px] w-full max-w-full lg:block" aria-hidden="true">
          <div className="absolute inset-x-[22%] bottom-[5%] h-[5%] rounded-[50%] bg-black/55 blur-md" />
          <Image src="/art/archer.png" alt="" fill sizes="40vw" className="object-contain object-bottom" />
        </div>
      </div>

      <div className="mt-16 grid gap-12 border-t border-border pt-12 md:grid-cols-2">
        <section>
          <h2 className="text-2xl font-bold text-parchment">{dict.download.stepsTitle}</h2>
          {/* A real sequence: install, sign up, play. */}
          <ol className="mt-6 grid gap-5">
            {dict.download.steps.map((step, index) => (
              <li key={step} className="flex gap-4">
                <span className="flex size-8 shrink-0 items-center justify-center rounded-full border border-gold/50 font-display font-bold text-gold">
                  {index + 1}
                </span>
                <span className="pt-1 text-parchment/85">{step}</span>
              </li>
            ))}
          </ol>
          <p className="mt-6 text-parchment/75">
            {dict.download.noAccount}{" "}
            <Link href={`/${lang}/register`} className="text-heraldic underline-offset-4 hover:underline">
              {dict.nav.register}
            </Link>
          </p>
        </section>
        <section>
          <h2 className="text-2xl font-bold text-parchment">{dict.download.requirementsTitle}</h2>
          <ul className="mt-6 grid gap-3 text-parchment/85">
            {dict.download.requirements.map((requirement) => (
              <li key={requirement} className="border-b border-border pb-3">
                {requirement}
              </li>
            ))}
          </ul>
        </section>
      </div>
    </div>
  );
}
