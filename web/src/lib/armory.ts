import "server-only";
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, rename, stat } from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";
import { pool } from "./db";

// The Armory's turning hero: the game client draws them, without a window,
// into a sheet of frames (`conquer-sandbox --armory`, client/crates/sandbox/
// src/armory.rs). A look is drawn once and kept on disk by what it wears,
// so the same look is never drawn twice.
//
// ARMORY_BIN    the client's executable (conquer-sandbox)
// ARMORY_ASSETS the game folder it reads (with gravak.gpk on the server)
// ARMORY_CACHE  where the sheets are kept (default .cache/armory)
// Without ARMORY_BIN the page keeps the profession's still art.

const run = promisify(execFile);

/** Frames in a sheet, side by side, and each one's size: armory.rs's. */
export const SHEET = { frames: 24, width: 400, height: 460 };

/** What a hero looks like, as the client draws them. */
type Look = { mesh: number; hair: number; armor: number; garment: number; helmet: number; right: number; left: number };

export function armoryEnabled(): boolean {
  return Boolean(process.env.ARMORY_BIN);
}

/** The look of a character by name, as the server shows them in the world (`MsgPlayer::from_character`). */
async function lookOf(name: string): Promise<Look | null> {
  const { rows } = await pool.query(
    `SELECT c.mesh, c.hair_style, e.position, e.item_type,
            COALESCE(s.look, 0) AS skin, COALESCE(k.color, 0) AS color
       FROM characters c
       LEFT JOIN equipment_items e ON e.character_id = c.character_id AND e.position IN (1, 3, 4, 5, 9)
       LEFT JOIN item_skins s ON s.item_uid = e.item_uid
       LEFT JOIN item_colors k ON k.item_uid = e.item_uid
      WHERE lower(c.name) = lower($1)`,
    [name],
  );
  if (rows.length === 0) return null;
  const at = (position: number) => rows.find((row) => Number(row.position) === position);
  // A Tinter dye folds into the id (`character::dyed`), 3 to 9.
  const dyed = (row: (typeof rows)[number] | undefined, look: number) => {
    const color = Number(row?.color ?? 0);
    return look > 0 && color >= 3 && color <= 9 ? look + color * 10_000_000 : look;
  };
  const worn = (position: number) => Number(at(position)?.item_type ?? 0);
  const weapon = (position: number) => {
    const row = at(position);
    return row ? Number(row.skin) || Number(row.item_type) : 0;
  };
  return {
    mesh: Number(rows[0].mesh),
    hair: Number(rows[0].hair_style),
    armor: dyed(at(3), worn(3)),
    garment: worn(9),
    helmet: dyed(at(1), worn(1)),
    right: weapon(4),
    left: dyed(at(5), weapon(5)),
  };
}

/** Sheets being drawn now, so two visitors of one look wait for one drawing. */
const drawing = new Map<string, Promise<string>>();

/** The sheet of `name`'s look: its file, drawn if it is new; `null` when no such character. */
export async function armorySheet(name: string): Promise<{ file: string; key: string } | null> {
  const bin = process.env.ARMORY_BIN;
  if (!bin) return null;
  const look = await lookOf(name);
  if (!look) return null;
  const args = [look.mesh, look.hair, look.armor, look.garment, look.helmet, look.right, look.left].map(String);
  const key = createHash("sha1").update(`${SHEET.frames}x${SHEET.width}x${SHEET.height}:${args.join(",")}`).digest("hex").slice(0, 16);
  const folder = process.env.ARMORY_CACHE ?? path.join(process.cwd(), ".cache", "armory");
  const file = path.join(folder, `${key}.png`);
  if (await exists(file)) return { file, key };
  let pending = drawing.get(key);
  if (!pending) {
    pending = (async () => {
      await mkdir(folder, { recursive: true });
      const partial = `${file}.${process.pid}.tmp.png`;
      const assets = process.env.ARMORY_ASSETS ? ["--assets", process.env.ARMORY_ASSETS] : [];
      await run(bin, ["--armory", partial, ...args, ...assets], { timeout: 30_000, windowsHide: true });
      await rename(partial, file);
      return file;
    })().finally(() => drawing.delete(key));
    drawing.set(key, pending);
  }
  return { file: await pending, key };
}

export async function readSheet(file: string): Promise<Buffer> {
  return readFile(file);
}

async function exists(file: string): Promise<boolean> {
  try {
    return (await stat(file)).size > 0;
  } catch {
    return false;
  }
}
