"use client";

import Link from "next/link";
import { useRef } from "react";

/** The client over the whole window, under a thin bar back to the site. */
export function GameFrame({ src, home, copy }: { src: string; home: string; copy: { back: string; fullscreen: string; testBuild: string; download: string; downloadHref: string } }) {
  const frame = useRef<HTMLIFrameElement>(null);
  return (
    <div className="fixed inset-0 z-50 flex flex-col bg-black">
      <div className="flex h-9 shrink-0 items-center gap-4 border-b border-white/10 bg-night px-3 text-sm">
        <Link href={home} className="text-parchment/80 hover:text-parchment">
          ← {copy.back}
        </Link>
        <span className="hidden text-slate-blue sm:inline">
          {copy.testBuild} ·{" "}
          <Link href={copy.downloadHref} className="text-heraldic underline-offset-4 hover:underline">
            {copy.download}
          </Link>
        </span>
        <button
          type="button"
          onClick={() => frame.current?.requestFullscreen?.().catch(() => {})}
          className="ml-auto rounded-md px-2 py-1 text-parchment/80 hover:bg-white/5 hover:text-parchment"
        >
          {copy.fullscreen}
        </button>
      </div>
      <iframe
        ref={frame}
        src={src}
        title="Gravak"
        className="w-full flex-1 border-0 bg-black"
        allow="fullscreen; autoplay; clipboard-read; clipboard-write"
        allowFullScreen
        // The game takes the keys at once.
        onLoad={() => frame.current?.focus()}
      />
    </div>
  );
}
