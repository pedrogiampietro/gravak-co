import Image from "next/image";
import type { Dictionary } from "@/i18n/dictionaries";

const DISCORD = process.env.NEXT_PUBLIC_DISCORD_URL;

export function SiteFooter({ dict }: { dict: Dictionary }) {
  return (
    <footer className="mt-24 border-t border-white/5">
      <div className="mx-auto flex max-w-6xl flex-col gap-4 px-4 py-10 text-sm text-slate-blue sm:flex-row sm:items-center sm:px-6">
        <Image src="/brand/crest.png" alt="" width={28} height={28} />
        <p className="max-w-prose">{dict.footer.note}</p>
        {DISCORD && (
          <a href={DISCORD} className="text-heraldic underline-offset-4 hover:underline sm:ml-auto">
            {dict.nav.discord}
          </a>
        )}
      </div>
    </footer>
  );
}
