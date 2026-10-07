import type { Metadata, Viewport } from "next";
import { Alegreya_Sans, Cinzel } from "next/font/google";
import { notFound } from "next/navigation";
import { SiteFooter } from "@/components/site-footer";
import { SiteHeader } from "@/components/site-header";
import { getDictionary, hasLocale, locales } from "@/i18n/dictionaries";
import "../globals.css";

// Cinzel is the face the game's own windows use for their titles.
const display = Cinzel({ variable: "--font-display", subsets: ["latin"], weight: ["600", "700"] });
const body = Alegreya_Sans({ variable: "--font-body", subsets: ["latin"], weight: ["400", "500", "700"] });

export const viewport: Viewport = { themeColor: "#0c1729", colorScheme: "dark" };

export function generateStaticParams() {
  return locales.map((lang) => ({ lang }));
}

export async function generateMetadata({ params }: LayoutProps<"/[lang]">): Promise<Metadata> {
  const { lang } = await params;
  if (!hasLocale(lang)) return {};
  const dict = getDictionary(lang);
  return {
    metadataBase: new URL(process.env.SITE_URL ?? "http://localhost:3000"),
    title: { default: dict.meta.title, template: "%s · Gravak" },
    description: dict.meta.description,
    icons: { icon: "/brand/crest.png" },
    openGraph: { title: dict.meta.title, description: dict.meta.description, images: ["/brand/og.png"] },
    alternates: { languages: { "pt-BR": "/pt", en: "/en" } },
  };
}

export default async function LocaleLayout({ children, params }: LayoutProps<"/[lang]">) {
  const { lang } = await params;
  if (!hasLocale(lang)) notFound();
  const dict = getDictionary(lang);
  return (
    <html lang={lang === "pt" ? "pt-BR" : "en"} className={`${display.variable} ${body.variable} antialiased`}>
      <body className="flex min-h-dvh flex-col">
        <a
          href="#content"
          className="sr-only z-50 rounded-md bg-gold px-4 py-2 font-bold text-primary-foreground focus:not-sr-only focus:fixed focus:top-3 focus:left-3"
        >
          {dict.nav.skip}
        </a>
        <SiteHeader lang={lang} dict={dict} />
        <main id="content" className="flex-1">
          {children}
        </main>
        <SiteFooter dict={dict} />
      </body>
    </html>
  );
}
