"use client";

import { useActionState } from "react";
import { Button } from "@/components/ui/button";
import type { Dictionary } from "@/i18n/dictionaries";
import { cn } from "@/lib/utils";
import { buy, type BuyState } from "./actions";

export type CharacterChoice = { id: number; label: string };
export type PackageChoice = { id: number; cps: string; price: string; bonus: number };

/** The character and the package, then Mercado Pago's Checkout Pro. */
export function BuyForm({
  lang,
  copy,
  characters,
  packages,
}: {
  lang: "pt" | "en";
  copy: Dictionary["shop"];
  characters: CharacterChoice[];
  packages: PackageChoice[];
}) {
  const [state, action, pending] = useActionState<BuyState, FormData>(buy, { status: "idle" });
  const error = state.status === "error" ? copy.errors[state.error] : undefined;
  return (
    <form action={action} className="mt-8 grid gap-8">
      <input type="hidden" name="lang" value={lang} />
      <fieldset className="grid gap-3">
        <legend className="mb-3 text-lg font-bold text-parchment">{copy.character}</legend>
        <div className="flex flex-wrap gap-3">
          {characters.map((character, index) => (
            <label
              key={character.id}
              className="flex cursor-pointer items-center gap-2 rounded-lg border border-border bg-night/60 px-4 py-3 has-checked:border-gold has-checked:bg-gold/10"
            >
              <input type="radio" name="character" value={character.id} defaultChecked={index === 0} className="accent-gold" />
              <span className="text-parchment">{character.label}</span>
            </label>
          ))}
        </div>
      </fieldset>
      <fieldset className="grid gap-3">
        <legend className="mb-3 text-lg font-bold text-parchment">{copy.package}</legend>
        <div className="grid grid-cols-[repeat(auto-fill,minmax(10rem,1fr))] gap-3">
          {packages.map((choice, index) => (
            <label
              key={choice.id}
              className={cn(
                "grid cursor-pointer gap-1 rounded-lg border border-border bg-night/60 p-4 text-center",
                "has-checked:border-gold has-checked:bg-gold/10 has-focus-visible:ring-2 has-focus-visible:ring-gold",
              )}
            >
              <input type="radio" name="package" value={choice.id} defaultChecked={index === 1} className="sr-only" />
              <span className="font-display text-2xl font-bold text-gold">{choice.cps} CPs</span>
              <span className="text-lg text-parchment">{choice.price}</span>
              <span className="min-h-5 text-sm text-heraldic">{choice.bonus > 0 ? `+${choice.bonus}% ${copy.bonus}` : ""}</span>
            </label>
          ))}
        </div>
      </fieldset>
      <p className="text-sm text-destructive empty:hidden" role="alert" aria-live="polite">
        {error}
      </p>
      <div>
        <Button type="submit" disabled={pending} className="h-12 px-7 text-base font-bold">
          {pending ? copy.paying : copy.pay}
        </Button>
        <p className="mt-3 max-w-[60ch] text-sm text-slate-blue">{copy.secure}</p>
      </div>
    </form>
  );
}
