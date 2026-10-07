import { NextResponse, type NextRequest } from "next/server";

const locales = ["pt", "en"];

/** Portuguese unless the browser prefers English over it. */
function preferred(request: NextRequest): string {
  const header = request.headers.get("accept-language") ?? "";
  const languages = header
    .split(",")
    .map((part) => {
      const [tag, quality] = part.trim().split(";q=");
      return { tag: tag.toLowerCase(), quality: quality ? Number(quality) : 1 };
    })
    .sort((a, b) => b.quality - a.quality);
  for (const { tag } of languages) {
    if (tag.startsWith("pt")) return "pt";
    if (tag.startsWith("en")) return "en";
  }
  return "pt";
}

export function proxy(request: NextRequest) {
  const { pathname } = request.nextUrl;
  if (locales.some((locale) => pathname === `/${locale}` || pathname.startsWith(`/${locale}/`))) return;
  request.nextUrl.pathname = `/${preferred(request)}${pathname}`;
  return NextResponse.redirect(request.nextUrl);
}

export const config = {
  // Pages only: not Next's files, the pictures, the brand or the installer.
  matcher: ["/((?!_next|art|brand|downloads|favicon.ico|.*\\.[a-z0-9]+$).*)"],
};
