"use client";

import { useActionState, useEffect } from "react";
import { Turnstile } from "@/components/turnstile";
import { Button } from "@/components/ui/button";
import type { Dictionary } from "@/i18n/dictionaries";
import { type DownloadState, requestDownload } from "./actions";

type Copy = Dictionary["download"];

/** The download button: the Turnstile check, then the signed link starts the download. */
export function DownloadForm({ lang, copy }: { lang: "pt" | "en"; copy: Copy }) {
  const [state, action, pending] = useActionState<DownloadState, FormData>(requestDownload, { status: "idle" });

  useEffect(() => {
    if (state.status === "ready") window.location.assign(state.url);
  }, [state]);

  return (
    <form action={action} className="mt-8 grid justify-items-start gap-4">
      <Turnstile lang={lang} reset={state} />
      <Button type="submit" disabled={pending} className="h-12 px-7 text-base font-bold">
        {pending ? copy.preparing : copy.button}
      </Button>
      <p className="text-sm empty:hidden" role="status" aria-live="polite">
        {state.status === "error" ? <span className="text-destructive">{copy.captcha}</span> : null}
        {state.status === "ready" ? <span className="text-parchment/80">{copy.started}</span> : null}
      </p>
    </form>
  );
}
