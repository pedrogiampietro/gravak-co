import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { Suspense } from "react";
import { Button, buttonVariants } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { type Dictionary, getDictionary, hasLocale, type Locale } from "@/i18n/dictionaries";
import { pool } from "@/lib/db";
import { dateTime, money, number } from "@/lib/format";
import { professionOf } from "@/lib/game";
import { getAccountOrders, getPackages } from "@/lib/orders";
import { currentAccount } from "@/lib/session";
import { cn } from "@/lib/utils";
import { logout } from "../login/actions";
import { BuyForm } from "./buy-form";

export async function generateMetadata({ params }: PageProps<"/[lang]/shop">): Promise<Metadata> {
  const { lang } = await params;
  return hasLocale(lang) ? { title: getDictionary(lang).shop.title } : {};
}

export default async function ShopPage({ params }: PageProps<"/[lang]/shop">) {
  const { lang } = await params;
  if (!hasLocale(lang)) notFound();
  const dict = getDictionary(lang);
  return (
    <div className="mx-auto max-w-6xl px-4 pt-14 sm:px-6">
      <h1 className="gold-leaf text-4xl font-bold sm:text-5xl">{dict.shop.title}</h1>
      <p className="mt-4 max-w-[62ch] text-lg text-parchment/80">{dict.shop.lead}</p>
      <Suspense>
        <Shop lang={lang} dict={dict} />
      </Suspense>
    </div>
  );
}

async function Shop({ lang, dict }: { lang: Locale; dict: Dictionary }) {
  const copy = dict.shop;
  const account = await currentAccount();
  if (!account) {
    return (
      <div className="mt-8">
        <p className="text-parchment/85">{copy.signInLead}</p>
        <Link href={`/${lang}/login?next=/${lang}/shop`} className={cn(buttonVariants(), "mt-5 h-11 px-6 text-base font-bold")}>
          {copy.signIn}
        </Link>
        <p className="mt-6 text-sm text-slate-blue">{copy.inGame}</p>
      </div>
    );
  }

  let data;
  try {
    const [characters, packages, orders] = await Promise.all([
      pool.query<{ character_id: string; name: string; level: number; current_class: number }>(
        "SELECT character_id, name, level, current_class FROM characters WHERE account_id = $1 ORDER BY level DESC, name",
        [account.id],
      ),
      getPackages(),
      getAccountOrders(account.id),
    ]);
    data = { characters: characters.rows, packages, orders };
  } catch (error) {
    console.error("shop failed", error);
    return <p className="mt-8 text-parchment/80">{copy.offline}</p>;
  }

  // The bonus against the cheapest package's CPs per real.
  const base = Math.min(...data.packages.map((choice) => choice.cps / choice.priceCents));
  const packages = data.packages.map((choice) => ({
    id: choice.id,
    cps: number(lang, choice.cps),
    price: money(lang, choice.priceCents),
    bonus: Math.round(((choice.cps / choice.priceCents) / base - 1) * 100),
  }));
  const characters = data.characters.map((row) => {
    const profession = professionOf(row.current_class);
    return {
      id: Number(row.character_id),
      label: `${row.name} · ${profession ? dict.ranking.professions[profession] : "—"} · ${copy.level} ${row.level}`,
    };
  });

  return (
    <>
      <form action={logout} className="mt-6 flex flex-wrap items-center gap-3 text-sm text-parchment/75">
        <input type="hidden" name="lang" value={lang} />
        <span>
          {copy.signedInAs} <strong className="text-parchment">{account.username}</strong>
        </span>
        <Button type="submit" variant="outline" size="sm">
          {copy.signOut}
        </Button>
      </form>
      {characters.length ? (
        <BuyForm lang={lang} copy={copy} characters={characters} packages={packages} />
      ) : (
        <p className="mt-8 text-parchment/85">{copy.noCharacters}</p>
      )}
      <p className="mt-6 text-sm text-slate-blue">{copy.inGame}</p>
      {data.orders.length ? (
        <section className="mt-14 border-t border-border pt-10">
          <h2 className="text-2xl font-bold text-parchment">{copy.history}</h2>
          <div className="mt-6 overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>{copy.columns.date}</TableHead>
                  <TableHead>{copy.columns.character}</TableHead>
                  <TableHead className="text-right">{copy.columns.cps}</TableHead>
                  <TableHead className="text-right">{copy.columns.price}</TableHead>
                  <TableHead>{copy.columns.status}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {data.orders.map((order) => {
                  const status = order.credited ? "credited" : order.status;
                  return (
                    <TableRow key={order.orderId}>
                      <TableCell className="text-parchment/80">{dateTime(lang, order.createdAt.getTime() / 1000)}</TableCell>
                      <TableCell>{order.characterName}</TableCell>
                      <TableCell className="text-right tabular-nums">{number(lang, order.cps)}</TableCell>
                      <TableCell className="text-right tabular-nums">{money(lang, order.priceCents)}</TableCell>
                      <TableCell className={status === "credited" ? "text-emerald-300" : "text-parchment/80"}>
                        {copy.statuses[status as keyof typeof copy.statuses] ?? status}
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </div>
        </section>
      ) : null}
    </>
  );
}
