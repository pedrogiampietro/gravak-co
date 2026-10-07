import Link from "next/link";
import { cn } from "@/lib/utils";

/** A row of filter chips, as links so each choice has its own address. */
export function Filters({ options }: { options: { href: string; label: string; active: boolean }[] }) {
  return (
    <div className="mt-5 flex flex-wrap gap-2 text-sm">
      {options.map((option) => (
        <Link
          key={option.href}
          href={option.href}
          aria-current={option.active ? "page" : undefined}
          className={cn(
            "rounded-full border px-3 py-1 transition-colors",
            option.active ? "border-gold/60 bg-gold/10 text-gold" : "border-border text-parchment/75 hover:text-parchment",
          )}
        >
          {option.label}
        </Link>
      ))}
    </div>
  );
}
