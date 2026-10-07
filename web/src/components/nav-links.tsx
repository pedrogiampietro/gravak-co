"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { cn } from "@/lib/utils";

type NavLink = { href: string; label: string };

/** The header's links, the current section lit (a character page belongs to none). */
export function NavLinks({ links, variant, pathname: given }: { links: NavLink[]; variant: "bar" | "strip"; pathname?: string }) {
  return given !== undefined ? <Links links={links} variant={variant} pathname={given} /> : <CurrentLinks links={links} variant={variant} />;
}

function CurrentLinks({ links, variant }: { links: NavLink[]; variant: "bar" | "strip" }) {
  return <Links links={links} variant={variant} pathname={usePathname()} />;
}

function Links({ links, variant, pathname }: { links: NavLink[]; variant: "bar" | "strip"; pathname: string }) {
  return links.map((link) => {
    const current = pathname === link.href || pathname.startsWith(`${link.href}/`);
    return (
      <Link
        key={link.href}
        href={link.href}
        aria-current={current ? "page" : undefined}
        className={cn(
          "rounded-md transition-colors",
          variant === "bar" ? "hidden px-2.5 py-2 lg:block" : "shrink-0 px-3 py-2",
          current ? "text-gold" : "text-parchment/80 hover:text-parchment",
        )}
      >
        {link.label}
      </Link>
    );
  });
}
