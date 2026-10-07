import "server-only";
import { cacheLife } from "next/cache";
import { pool } from "./db";
import { type Profession, professionRange } from "./game";

export const categories = ["level", "nobility", "pk", "ko", "guilds", "wealth"] as const;
export type Category = (typeof categories)[number];

export const isCategory = (value: string | undefined): value is Category =>
  !!value && (categories as readonly string[]).includes(value);

export type Row = {
  id: number;
  name: string;
  currentClass: number;
  level: number;
  rebirths: number;
  value: number;
  /** Guilds: members and the leader's name. */
  members?: number;
  leader?: string;
};

const LIMIT = 50;

// The in-game Highscores' queries (server `highscores.rs`), so the site and
// the game rank alike.
const characterQueries: Record<Exclude<Category, "guilds">, { value: string; where: string; order: string }> = {
  level: { value: "rebirths", where: "TRUE", order: "rebirths DESC, level DESC, experience DESC, character_id" },
  nobility: { value: "nobility_donation", where: "nobility_donation > 0", order: "nobility_donation DESC, character_id" },
  pk: { value: "kill_points", where: "kill_points > 0", order: "kill_points DESC, character_id" },
  ko: { value: "best_ko", where: "best_ko > 0", order: "best_ko DESC, level DESC, character_id" },
  wealth: { value: "silver", where: "silver > 0", order: "silver DESC, character_id" },
};

/** The best of a category, `null` when the database does not answer. */
export async function getRanking(category: Category, profession: Profession | null): Promise<Row[] | null> {
  "use cache";
  cacheLife("minutes");
  try {
    if (category === "guilds") {
      const { rows } = await pool.query(
        `SELECT g.guild_id AS id, g.name, l.current_class AS "currentClass", l.level, l.rebirths,
                g.silver AS value, COUNT(m.character_id)::int AS members, l.name AS leader
           FROM guilds g
           JOIN characters l ON l.character_id = g.leader_id
           LEFT JOIN guild_members m ON m.guild_id = g.guild_id
          GROUP BY g.guild_id, g.name, l.current_class, l.level, l.rebirths, g.silver, l.name
          ORDER BY g.silver DESC, COUNT(m.character_id) DESC, g.name
          LIMIT $1`,
        [LIMIT],
      );
      return rows.map(normalize);
    }
    const query = characterQueries[category];
    const range = profession ? professionRange[profession] : [0, 1_000];
    const { rows } = await pool.query(
      `SELECT character_id AS id, name, current_class AS "currentClass", level, rebirths, ${query.value} AS value
         FROM characters
        WHERE ${query.where} AND current_class BETWEEN $1 AND $2
        ORDER BY ${query.order}
        LIMIT $3`,
      [range[0], range[1], LIMIT],
    );
    return rows.map(normalize);
  } catch (error) {
    console.error("ranking query failed", error);
    return null;
  }
}

function normalize(row: Record<string, unknown>): Row {
  return {
    id: Number(row.id),
    name: String(row.name),
    currentClass: Number(row.currentClass),
    level: Number(row.level),
    rebirths: Number(row.rebirths),
    value: Number(row.value),
    members: row.members === undefined ? undefined : Number(row.members),
    leader: row.leader === undefined ? undefined : String(row.leader),
  };
}
