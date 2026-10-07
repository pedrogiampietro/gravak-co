"use client";

import Link from "next/link";
import { useActionState, useEffect, useRef } from "react";
import { Button, buttonVariants } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { Dictionary } from "@/i18n/dictionaries";
import { cn } from "@/lib/utils";
import { type ErrorCode, register, type RegisterState } from "./actions";

type Copy = Dictionary["register"];

export function RegisterForm({ lang, copy }: { lang: "pt" | "en"; copy: Copy }) {
  const [state, action, pending] = useActionState<RegisterState, FormData>(register, { status: "idle" });
  const form = useRef<HTMLFormElement>(null);

  // After a refused submit, the first field in error takes the focus.
  useEffect(() => {
    if (state.status === "error") {
      form.current?.querySelector<HTMLInputElement>("[aria-invalid=true]")?.focus();
    }
  }, [state]);

  if (state.status === "created") {
    return (
      <div className="mt-8 rounded-lg border border-gold/40 bg-gold/5 p-6" role="status">
        <p className="text-parchment">
          {copy.created} <strong className="text-gold">{state.username}</strong>.
        </p>
        <Link href={`/${lang}/download`} className={cn(buttonVariants(), "mt-5 h-11 px-5 text-base font-bold")}>
          {copy.nextDownload}
        </Link>
      </div>
    );
  }

  const errors = state.status === "error" ? state.errors : {};
  const values = state.status === "error" ? state.values : { username: "", email: "" };
  const message = (code: ErrorCode | undefined) => (code ? copy.errors[code] : undefined);

  return (
    <form ref={form} action={action} className="mt-8 grid gap-5" noValidate>
      <Field
        id="username"
        label={copy.username}
        hint={copy.usernameHint}
        error={message(errors.username)}
        input={{ name: "username", autoComplete: "username", defaultValue: values.username, maxLength: 16, required: true, spellCheck: false, autoCapitalize: "none" }}
      />
      <Field
        id="password"
        label={copy.password}
        hint={copy.passwordHint}
        error={message(errors.password)}
        input={{ name: "password", type: "password", autoComplete: "new-password", minLength: 6, required: true }}
      />
      <Field
        id="confirm"
        label={copy.confirm}
        error={message(errors.confirm)}
        input={{ name: "confirm", type: "password", autoComplete: "new-password", required: true }}
      />
      <Field
        id="email"
        label={copy.email}
        hint={copy.emailHint}
        error={message(errors.email)}
        input={{ name: "email", type: "email", autoComplete: "email", defaultValue: values.email, maxLength: 64, spellCheck: false }}
      />
      <p className="text-sm text-destructive empty:hidden" role="alert" aria-live="polite">
        {errors.form ? copy.errors[errors.form] : null}
      </p>
      <Button type="submit" disabled={pending} className="mt-1 h-11 w-full text-base font-bold sm:w-auto sm:px-8">
        {pending ? copy.submitting : copy.submit}
      </Button>
    </form>
  );
}

function Field({
  id,
  label,
  hint,
  error,
  input,
}: {
  id: string;
  label: string;
  hint?: string;
  error?: string;
  input: React.ComponentProps<"input">;
}) {
  const describedBy = [hint && `${id}-hint`, error && `${id}-error`].filter(Boolean).join(" ") || undefined;
  return (
    <div className="grid gap-1.5">
      <Label htmlFor={id} className="text-[0.95rem] text-parchment">
        {label}
      </Label>
      <Input id={id} aria-invalid={error ? true : undefined} aria-describedby={describedBy} className="h-11 bg-night/60 text-base" {...input} />
      {error ? (
        <p id={`${id}-error`} className="text-sm text-destructive">
          {error}
        </p>
      ) : hint ? (
        <p id={`${id}-hint`} className="text-sm text-slate-blue">
          {hint}
        </p>
      ) : null}
    </div>
  );
}
