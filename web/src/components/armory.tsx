import Image from "next/image";
import type { Dictionary } from "@/i18n/dictionaries";
import type { Worn } from "@/lib/characters";
import { cn } from "@/lib/utils";

// The Armory: the hero in the middle, what they wear on either side, each
// piece telling all about itself on hover or focus, as the game's tooltip.

/** Which slots stand on each side: the left the hero's right hand's. */
const LEFT = [1, 2, 3, 9, 4, 15];
const RIGHT = [6, 7, 8, 5, 16];

export const qualityTone: Record<Worn["quality"], string> = {
  Super: "text-gold",
  Elite: "text-violet-300",
  Unique: "text-sky-300",
  Refined: "text-emerald-300",
  Normal: "text-parchment",
};

/** Rare .. Mythic: the colour of their name and of their frame. */
const rarityTone = ["", "text-sky-400", "text-fuchsia-400", "text-amber-400", "text-rose-400"];
const rarityFrame = [
  "border-border",
  "border-sky-400/70 shadow-[0_0_12px_rgb(56_189_248/0.35)]",
  "border-fuchsia-400/70 shadow-[0_0_12px_rgb(232_121_249/0.35)]",
  "border-amber-400/80 shadow-[0_0_14px_rgb(251_191_36/0.45)]",
  "border-rose-400/80 shadow-[0_0_16px_rgb(251_113_133/0.5)]",
];

/** Affixes counted in percent (server/game/src/affixes.rs); the others in points. */
const PERCENT_AFFIXES = new Set([1, 2, 3, 7, 8]);

export function slotName(copy: Dictionary["character"], position: number): string {
  if (position === 15 || position === 16) return copy.accessories[position - 15];
  return copy.slots[position - 1] ?? String(position);
}

export function Armory({
  equipment,
  copy,
  hero,
  label,
}: {
  equipment: Worn[];
  copy: Dictionary["character"];
  /** What stands in the middle: the hero, still or turning. */
  hero: React.ReactNode;
  label: string;
}) {
  const column = (positions: number[], side: "left" | "right") => (
    <ul className="flex flex-col items-center gap-2.5 sm:gap-3">
      {positions.map((position) => (
        <Slot key={position} item={equipment.find((worn) => worn.position === position)} name={slotName(copy, position)} copy={copy} side={side} />
      ))}
    </ul>
  );
  return (
    <figure
      aria-label={label}
      className="relative grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3 rounded-xl border border-border bg-[radial-gradient(ellipse_at_50%_40%,rgb(212_175_55/0.12),transparent_65%)] bg-card/40 p-3 sm:gap-6 sm:p-6"
    >
      {column(LEFT, "left")}
      <div className="relative h-full min-h-[340px] sm:min-h-[460px]">{hero}</div>
      {column(RIGHT, "right")}
    </figure>
  );
}

function Slot({ item, name, copy, side }: { item: Worn | undefined; name: string; copy: Dictionary["character"]; side: "left" | "right" }) {
  if (!item) {
    return (
      <li
        title={`${name}: ${copy.emptySlot}`}
        className="size-12 rounded-md border border-dashed border-parchment/20 bg-night/60 shadow-[inset_0_0_12px_rgb(0_0_0/0.6)] sm:size-14"
        aria-label={`${name}: ${copy.emptySlot}`}
      />
    );
  }
  const title = `${item.quality !== "Normal" ? `${item.quality} ` : ""}${item.name}${item.plus > 0 ? ` +${item.plus}` : ""}`;
  return (
    <li className="group relative">
      <button
        type="button"
        aria-label={`${name}: ${title}`}
        className={cn(
          "relative block size-12 rounded-md border-2 bg-night/70 outline-none transition-transform hover:scale-105 focus-visible:scale-105 focus-visible:ring-2 focus-visible:ring-gold sm:size-14",
          rarityFrame[item.rarity] ?? rarityFrame[0],
        )}
      >
        {item.icon && <Image src={`/items/${item.icon}.png`} alt="" fill unoptimized sizes="56px" className="object-contain p-1" />}
        {item.plus > 0 && (
          <span className="absolute top-0 right-0.5 font-display text-xs font-bold text-[#ffe65a] [text-shadow:0_0_2px_#000,0_0_2px_#000]">
            +{item.plus}
          </span>
        )}
      </button>
      <Tooltip item={item} name={name} title={title} copy={copy} side={side} />
    </li>
  );
}

function Tooltip({
  item,
  name,
  title,
  copy,
  side,
}: {
  item: Worn;
  name: string;
  title: string;
  copy: Dictionary["character"];
  side: "left" | "right";
}) {
  const tip = copy.tooltip;
  const { stats } = item;
  const lines: [string, string][] = [];
  if (stats.attack[1] > 0) lines.push([tip.attack, `${stats.attack[0]} - ${stats.attack[1]}`]);
  if (stats.magicAttack > 0) lines.push([tip.magicAttack, String(stats.magicAttack)]);
  if (stats.defense > 0) lines.push([tip.defense, String(stats.defense)]);
  if (stats.magicDefense > 0) lines.push([tip.magicDefense, String(stats.magicDefense)]);
  if (stats.life > 0) lines.push([tip.life, `+${stats.life}`]);
  if (stats.mana > 0) lines.push([tip.mana, `+${stats.mana}`]);
  if (stats.accuracy > 0) lines.push([tip.accuracy, `+${stats.accuracy}`]);
  if (stats.dodge > 0) lines.push([tip.dodge, `+${stats.dodge}`]);
  return (
    <div
      role="tooltip"
      className={cn(
        "pointer-events-none invisible absolute top-0 z-30 w-64 rounded-md border border-gold/40 bg-[#0b0f1a]/95 p-3 text-left text-[0.82rem] leading-snug opacity-0 shadow-[0_12px_32px_rgb(0_0_0/0.6)] backdrop-blur-sm transition-opacity duration-150",
        "group-hover:visible group-hover:opacity-100 group-focus-within:visible group-focus-within:opacity-100",
        side === "left" ? "left-full ml-3" : "right-full mr-3",
      )}
    >
      <p className={cn("font-display text-[0.95rem] font-bold", item.rarity > 0 ? rarityTone[item.rarity] : qualityTone[item.quality])}>{title}</p>
      <p className="text-xs text-slate-blue">
        {item.rarity > 0 && <span className={cn("font-semibold", rarityTone[item.rarity])}>{tip.rarities[item.rarity]} · </span>}
        {item.kind.replaceAll("`", "'") || name}
      </p>
      {item.level > 0 && (
        <p className="mt-2 text-parchment/80">
          {tip.level}: <span className="text-parchment">{item.level}</span>
        </p>
      )}
      {lines.length > 0 && (
        <dl className="mt-2 grid grid-cols-[1fr_auto] gap-x-3 gap-y-0.5 tabular-nums">
          {lines.map(([label, value]) => (
            <div key={label} className="contents">
              <dt className="text-parchment/75">{label}</dt>
              <dd className="text-right text-parchment">{value}</dd>
            </div>
          ))}
        </dl>
      )}
      {(item.bless > 0 || item.enchant > 0) && (
        <p className="mt-2 grid gap-0.5 text-emerald-300">
          {item.bless > 0 && (
            <span>
              {tip.bless} -{item.bless}%
            </span>
          )}
          {item.enchant > 0 && (
            <span>
              {tip.enchant} +{item.enchant}
            </span>
          )}
        </p>
      )}
      {item.affixes.length > 0 && (
        <ul className={cn("mt-2 grid gap-0.5", rarityTone[item.rarity])}>
          {item.affixes.map((affix, index) => (
            <li key={`${affix.kind}-${index}`}>
              {tip.affixes[affix.kind]} {affix.kind === 7 ? "-" : "+"}
              {affix.value}
              {PERCENT_AFFIXES.has(affix.kind) ? "%" : ""}
            </li>
          ))}
        </ul>
      )}
      {item.sockets.length > 0 && (
        <ul className="mt-2 grid gap-1">
          {item.sockets.map((socket, index) => (
            <li key={index} className="flex items-center gap-2">
              <span className="relative size-5 shrink-0 rounded-full border border-border bg-night">
                {socket.icon && <Image src={`/items/${socket.icon}.png`} alt="" fill unoptimized sizes="20px" className="object-contain" />}
              </span>
              <span className={socket.name ? "text-amber-200" : "text-slate-blue"}>{socket.name ?? tip.openSocket}</span>
            </li>
          ))}
        </ul>
      )}
      {item.durability && (
        <p className="mt-2 text-xs text-parchment/70 tabular-nums">
          {tip.durability}: {item.durability[0]}/{item.durability[1]}
        </p>
      )}
      {item.description && <p className="mt-2 border-t border-border pt-2 text-xs text-parchment/60">{item.description}</p>}
    </div>
  );
}
