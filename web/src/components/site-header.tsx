import Image from "next/image";
import Link from "next/link";
import type { Dictionary, Locale } from "@/i18n/dictionaries";
import { Suspense } from "react";
import { NavLinks } from "./nav-links";
import { TopBar } from "./top-bar";
import { buttonVariants } from "./ui/button";

export function SiteHeader({ lang, dict }: { lang: Locale; dict: Dictionary }) {
  const links = [
    { href: `/${lang}/news`, label: dict.nav.news },
    { href: `/${lang}/ranking`, label: dict.nav.ranking },
    { href: `/${lang}/powergamers`, label: dict.community.powergamers },
    { href: `/${lang}/killers`, label: dict.community.killers },
    { href: `/${lang}/guilds`, label: dict.community.guilds },
    { href: `/${lang}/download`, label: dict.nav.download },
  ];
  return (
    <header className="sticky top-0 z-30 bg-night/85 backdrop-blur-md">
      <TopBar lang={lang} dict={dict} />
      <div className="border-b border-white/5">
        <div className="mx-auto flex h-16 max-w-6xl items-center gap-6 px-4 sm:px-6">
          <Link href={`/${lang}`} className="flex items-center gap-2.5" aria-label={`Gravak — ${dict.nav.home}`}>
            <Image src="/brand/crest.png" alt="" width={34} height={34} priority />
            <span translate="no" className="gold-leaf font-display text-xl font-bold tracking-[0.18em]">
              GRAVAK
            </span>
          </Link>
          <nav className="ml-auto flex items-center gap-0.5 text-[0.95rem]" aria-label="Principal">
            <Suspense fallback={<NavLinks links={links} variant="bar" pathname="" />}>
              <NavLinks links={links} variant="bar" />
            </Suspense>
            <Link href={`/${lang}/register`} className={buttonVariants({ size: "sm" })}>
              {dict.nav.register}
            </Link>
          </nav>
        </div>
        {/* On phones and tablets the links move under the bar and scroll. */}
        <nav className="flex gap-1 overflow-x-auto border-t border-white/5 px-2 text-sm whitespace-nowrap lg:hidden" aria-label="Principal">
          <Suspense fallback={<NavLinks links={links} variant="strip" pathname="" />}>
            <NavLinks links={links} variant="strip" />
          </Suspense>
        </nav>
      </div>
    </header>
  );
}
