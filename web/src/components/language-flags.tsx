"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { cn } from "@/lib/utils";

const languages = [
  { code: "pt", name: "Português", Flag: Brazil },
  { code: "en", name: "English", Flag: UnitedStates },
] as const;

/** The same page in each language, the current one lit. */
export function LanguageFlags({ lang, className, pathname: given }: { lang: "pt" | "en"; className?: string; pathname?: string }) {
  return given ? <Flags lang={lang} className={className} pathname={given} /> : <CurrentFlags lang={lang} className={className} />;
}

function CurrentFlags({ lang, className }: { lang: "pt" | "en"; className?: string }) {
  return <Flags lang={lang} className={className} pathname={usePathname()} />;
}

function Flags({ lang, className, pathname }: { lang: "pt" | "en"; className?: string; pathname: string }) {
  return (
    <span className={cn("flex items-center gap-1.5", className)}>
      {languages.map(({ code, name, Flag }) => (
        <Link
          key={code}
          href={pathname.replace(/^\/(pt|en)(?=\/|$)/, `/${code}`)}
          hrefLang={code === "pt" ? "pt-BR" : "en"}
          aria-label={name}
          aria-current={code === lang ? "true" : undefined}
          className={cn(
            "rounded-[3px] p-0.5 transition-[opacity,box-shadow]",
            code === lang ? "opacity-100 ring-1 ring-gold" : "opacity-55 hover:opacity-100",
          )}
        >
          <Flag />
        </Link>
      ))}
    </span>
  );
}

function Brazil() {
  return (
    <svg viewBox="0 0 28 20" className="block h-3.5 w-5" aria-hidden="true">
      <rect width="28" height="20" fill="#009c3b" />
      <path d="M14 2.5 25.5 10 14 17.5 2.5 10Z" fill="#ffdf00" />
      <circle cx="14" cy="10" r="4.6" fill="#002776" />
      <path d="M9.6 9.1a9 9 0 0 1 8.8 1.6" stroke="#fff" strokeWidth="0.9" fill="none" />
    </svg>
  );
}

function UnitedStates() {
  return (
    <svg viewBox="0 0 28 20" className="block h-3.5 w-5" aria-hidden="true">
      <rect width="28" height="20" fill="#fff" />
      {[0, 2, 4, 6, 8, 10, 12].map((row) => (
        <rect key={row} y={(row * 20) / 13} width="28" height={20 / 13} fill="#b22234" />
      ))}
      <rect width="12" height={(20 / 13) * 7} fill="#3c3b6e" />
    </svg>
  );
}
