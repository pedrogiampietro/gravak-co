"use client";

import Script from "next/script";
import { useCallback, useEffect, useRef } from "react";

declare global {
  interface Window {
    turnstile?: {
      render: (container: HTMLElement, options: Record<string, unknown>) => string;
      reset: (widget: string) => void;
      remove: (widget: string) => void;
    };
  }
}

/** The key of the Cloudflare Turnstile widget; without it no widget is drawn. */
export const TURNSTILE_SITE_KEY = process.env.NEXT_PUBLIC_TURNSTILE_SITE_KEY ?? "";

/**
 * Cloudflare's Turnstile check. Inside a form it adds the hidden
 * `cf-turnstile-response` field the server action verifies. A token works
 * once, so the widget asks for a new one whenever `reset` changes (pass the
 * action's state: each answer from the server).
 */
export function Turnstile({ lang, reset }: { lang: "pt" | "en"; reset?: unknown }) {
  const box = useRef<HTMLDivElement>(null);
  const widget = useRef<string | undefined>(undefined);

  const render = useCallback(() => {
    if (!box.current || !window.turnstile || widget.current !== undefined) return;
    widget.current = window.turnstile.render(box.current, {
      sitekey: TURNSTILE_SITE_KEY,
      theme: "dark",
      language: lang === "pt" ? "pt-br" : "en",
    });
  }, [lang]);

  // The script may already be there from another page.
  useEffect(() => {
    render();
    return () => {
      if (widget.current !== undefined) window.turnstile?.remove(widget.current);
      widget.current = undefined;
    };
  }, [render]);

  useEffect(() => {
    if (widget.current !== undefined) window.turnstile?.reset(widget.current);
  }, [reset]);

  if (!TURNSTILE_SITE_KEY) return null;
  return (
    <>
      <Script src="https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit" onReady={render} />
      <div ref={box} className="min-h-[65px]" />
    </>
  );
}
