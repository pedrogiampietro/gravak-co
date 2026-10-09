"use client";

import Link from "next/link";
import { useActionState } from "react";
import { Turnstile } from "@/components/turnstile";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { Dictionary } from "@/i18n/dictionaries";
import { login, type LoginState } from "./actions";

export function LoginForm({ lang, copy, next, registerLabel }: { lang: "pt" | "en"; copy: Dictionary["login"]; next: string; registerLabel: string }) {
  const [state, action, pending] = useActionState<LoginState, FormData>(login, { status: "idle" });
  const error = state.status === "error" ? copy.errors[state.error] : undefined;
  return (
    <form action={action} className="mt-8 grid gap-5">
      <input type="hidden" name="lang" value={lang} />
      <input type="hidden" name="next" value={next} />
      <div className="grid gap-1.5">
        <Label htmlFor="username" className="text-[0.95rem] text-parchment">
          {copy.username}
        </Label>
        <Input
          id="username"
          name="username"
          autoComplete="username"
          defaultValue={state.status === "error" ? state.username : ""}
          maxLength={16}
          required
          spellCheck={false}
          autoCapitalize="none"
          className="h-11 bg-night/60 text-base"
        />
      </div>
      <div className="grid gap-1.5">
        <Label htmlFor="password" className="text-[0.95rem] text-parchment">
          {copy.password}
        </Label>
        <Input id="password" name="password" type="password" autoComplete="current-password" required className="h-11 bg-night/60 text-base" />
      </div>
      <Turnstile lang={lang} reset={state} />
      <p className="text-sm text-destructive empty:hidden" role="alert" aria-live="polite">
        {error}
      </p>
      <Button type="submit" disabled={pending} className="h-11 w-full text-base font-bold sm:w-auto sm:px-8">
        {pending ? copy.submitting : copy.submit}
      </Button>
      <p className="text-parchment/75">
        {copy.noAccount}{" "}
        <Link href={`/${lang}/register`} className="text-heraldic underline-offset-4 hover:underline">
          {registerLabel}
        </Link>
      </p>
    </form>
  );
}
