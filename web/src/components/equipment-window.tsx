import Image from "next/image";
import layout from "@/content/equipment-window.json";
import type { Worn } from "@/lib/characters";
import type { Profession } from "@/lib/game";

// The client's Character window on its Equipment tab (`shop::status_base_image`),
// exported empty with its slots (`site_items`); everything is placed in
// percent of its 264x440 pixels so it scales.
const [WIDTH, HEIGHT] = layout.size;
const percent = ([x, y, width, height]: number[]) => ({
  left: `${(x / WIDTH) * 100}%`,
  top: `${(y / HEIGHT) * 100}%`,
  width: `${(width / WIDTH) * 100}%`,
  height: `${(height / HEIGHT) * 100}%`,
});
// Where the hero stands on the paper, and the level orb at the top left.
const HERO = [72, 58, 118, 180];
const ORB = [6, 6, 40, 40];

export function EquipmentWindow({
  equipment,
  level,
  profession,
  slotNames,
  label,
}: {
  equipment: Worn[];
  level: number;
  profession: Profession | null;
  slotNames: readonly string[];
  label: string;
}) {
  return (
    <figure
      aria-label={label}
      className="relative aspect-[264/440] w-full max-w-[396px] select-none drop-shadow-[0_20px_40px_rgb(0_0_0/0.5)]"
    >
      <Image src="/ui/equipment-window.png" alt="" fill sizes="396px" unoptimized priority className="[image-rendering:auto]" />
      <span
        className="absolute flex items-center justify-center font-display font-bold text-parchment [text-shadow:0_1px_2px_#000] tabular-nums"
        style={{ ...percent(ORB), fontSize: "clamp(0.8rem, 3.6vw, 1.15rem)" }}
      >
        {level}
      </span>
      {profession && (
        <div className="absolute" style={percent(HERO)}>
          <Image src={`/art/${profession}.png`} alt="" fill sizes="180px" className="object-contain object-bottom opacity-95" />
        </div>
      )}
      {layout.slots.map((slot) => {
        const item = equipment.find((worn) => worn.position === slot.position);
        if (!item) return null;
        const name = `${item.quality !== "Normal" ? `${item.quality} ` : ""}${item.name}${item.plus > 0 ? ` +${item.plus}` : ""}`;
        const big = slot.rect[2] >= 60;
        return (
          <div
            key={slot.position}
            className="group absolute"
            style={percent(slot.rect)}
            title={`${slotNames[slot.position - 1]}: ${name}`}
          >
            {item.icon && (
              <Image
                src={`/items/${item.icon}.png`}
                alt={name}
                fill
                unoptimized
                sizes={big ? "98px" : "60px"}
                className="object-contain p-[5%]"
              />
            )}
            {item.plus > 0 && (
              <span
                className="absolute top-0 right-[4%] font-display font-bold text-[#ffe65a] [text-shadow:-1px_-1px_0_#140a00,1px_-1px_0_#140a00,-1px_1px_0_#140a00,1px_1px_0_#140a00]"
                style={{ fontSize: big ? "clamp(0.7rem, 3vw, 0.95rem)" : "clamp(0.6rem, 2.6vw, 0.8rem)" }}
              >
                +{item.plus}
              </span>
            )}
          </div>
        );
      })}
    </figure>
  );
}
