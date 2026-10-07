import "server-only";
import { cacheLife } from "next/cache";
import items from "@/content/items.json";
import { pool } from "./db";
import { type NobilityRank, nobilityRank, type Profession, professionRange } from "./game";

/** Online: the server marks those in the world every 30 seconds. */
const ONLINE_WITHIN = 90;

const catalogue = items as unknown as Record<string, [string, number, string]>;

export type Worn = {
  position: number;
  itemType: number;
  name: string;
  level: number;
  icon: string | null;
  plus: number;
  quality: "Normal" | "Refined" | "Unique" | "Elite" | "Super";
};

export type Death = {
  at: number;
  level: number;
  /** The other side: who killed (a death) or who was killed (a kill). */
  name: string;
  /** A player's character, linked; null for a monster. */
  character: string | null;
  otherLevel: number | null;
};

export type Character = {
  name: string;
  currentClass: number;
  level: number;
  rebirths: number;
  female: boolean;
  online: boolean;
  lastSeen: number;
  createdAt: number;
  pkPoints: number;
  bestKo: number;
  nobility: NobilityRank;
  nobilityPlace: number | null;
  spouse: string | null;
  guild: { name: string; role: number } | null;
  equipment: Worn[];
  deaths: Death[];
  kills: Death[];
  history: { day: string; gained: number; level: number }[];
};

function quality(itemType: number): Worn["quality"] {
  switch (itemType % 10) {
    case 9:
      return "Super";
    case 8:
      return "Elite";
    case 7:
      return "Unique";
    case 6:
      return "Refined";
    default:
      return "Normal";
  }
}

/** A character by name (any case), `null` when none, `undefined` when the database is down. */
export async function getCharacter(name: string): Promise<Character | null | undefined> {
  "use cache";
  cacheLife("minutes");
  try {
    const { rows } = await pool.query(
      `SELECT c.character_id, c.name, c.current_class, c.level, c.rebirths, c.mesh, c.kill_points, c.best_ko,
              c.nobility_donation, c.last_seen, c.created_at,
              EXTRACT(EPOCH FROM now())::BIGINT - c.last_seen AS idle,
              m.spouse_name, g.name AS guild, gm.role
         FROM characters c
         LEFT JOIN marriages m ON m.character_id = c.character_id
         LEFT JOIN guild_members gm ON gm.character_id = c.character_id
         LEFT JOIN guilds g ON g.guild_id = gm.guild_id
        WHERE lower(c.name) = lower($1)`,
      [name],
    );
    const row = rows[0];
    if (!row) return null;
    const id = Number(row.character_id);
    const donation = Number(row.nobility_donation);

    const [place, equipment, deaths, kills, history] = await Promise.all([
      donation > 0
        ? pool.query(
            "SELECT COUNT(*)::int + 1 AS place FROM characters WHERE nobility_donation > $1 OR (nobility_donation = $1 AND character_id < $2)",
            [donation, id],
          )
        : null,
      pool.query(
        `SELECT e.position, e.item_type, COALESCE(u.plus, 0) AS plus
           FROM equipment_items e LEFT JOIN item_upgrades u ON u.item_uid = e.item_uid
          WHERE e.character_id = $1 ORDER BY e.position`,
        [id],
      ),
      pool.query(
        `SELECT d.died_at, d.level, d.killer_name AS name, k.name AS character, d.killer_level AS other_level
           FROM character_deaths d LEFT JOIN characters k ON k.character_id = d.killer_id
          WHERE d.character_id = $1 ORDER BY d.died_at DESC LIMIT 10`,
        [id],
      ),
      pool.query(
        `SELECT d.died_at, d.killer_level AS level, v.name, v.name AS character, d.level AS other_level
           FROM character_deaths d JOIN characters v ON v.character_id = d.character_id
          WHERE d.killer_id = $1 ORDER BY d.died_at DESC LIMIT 10`,
        [id],
      ),
      pool.query(
        `SELECT to_char(day, 'YYYY-MM-DD') AS day, GREATEST(end_total - start_total, 0) AS gained, end_level
           FROM experience_days WHERE character_id = $1 ORDER BY day DESC LIMIT 7`,
        [id],
      ),
    ]);

    const nobilityPlace = place ? Number(place.rows[0].place) : null;
    return {
      name: String(row.name),
      currentClass: Number(row.current_class),
      level: Number(row.level),
      rebirths: Number(row.rebirths),
      female: Number(row.mesh) % 10_000 >= 2_000,
      online: Number(row.last_seen) > 0 && Number(row.idle) <= ONLINE_WITHIN,
      lastSeen: Number(row.last_seen),
      createdAt: Number(row.created_at),
      pkPoints: Number(row.kill_points),
      bestKo: Number(row.best_ko),
      nobility: nobilityPlace ? nobilityRank(nobilityPlace, donation) : nobilityRank(Number.MAX_SAFE_INTEGER, donation),
      nobilityPlace,
      spouse: row.spouse_name ? String(row.spouse_name) : null,
      guild: row.guild ? { name: String(row.guild), role: Number(row.role) } : null,
      equipment: equipment.rows.map((item) => {
        const itemType = Number(item.item_type);
        const known = catalogue[String(itemType)];
        return {
          position: Number(item.position),
          itemType,
          name: known?.[0] ?? `#${itemType}`,
          level: known?.[1] ?? 0,
          icon: known?.[2] ?? null,
          plus: Number(item.plus),
          quality: quality(itemType),
        };
      }),
      deaths: deaths.rows.map(death),
      kills: kills.rows.map(death),
      history: history.rows.map((day) => ({ day: String(day.day), gained: Number(day.gained), level: Number(day.end_level) })),
    };
  } catch (error) {
    console.error("character query failed", error);
    return undefined;
  }
}

function death(row: Record<string, unknown>): Death {
  return {
    at: Number(row.died_at),
    level: Number(row.level ?? 0),
    name: String(row.name),
    character: row.character ? String(row.character) : null,
    otherLevel: row.other_level === null || row.other_level === undefined ? null : Number(row.other_level),
  };
}

export type Gainer = { name: string; currentClass: number; level: number; rebirths: number; gained: number };

/** The most experience gained over the last `days` days (today counts as one). */
export async function getPowergamers(days: number, profession: Profession | null): Promise<Gainer[] | null> {
  "use cache";
  cacheLife("minutes");
  const range = profession ? professionRange[profession] : [0, 1_000];
  try {
    const { rows } = await pool.query(
      `SELECT c.name, c.current_class, c.level, c.rebirths, SUM(GREATEST(x.end_total - x.start_total, 0))::bigint AS gained
         FROM experience_days x JOIN characters c ON c.character_id = x.character_id
        WHERE x.day > (now() AT TIME ZONE 'UTC')::date - $1::int AND c.current_class BETWEEN $2 AND $3
        GROUP BY c.character_id
       HAVING SUM(GREATEST(x.end_total - x.start_total, 0)) > 0
        ORDER BY gained DESC, c.name LIMIT 50`,
      [days, range[0], range[1]],
    );
    return rows.map((row) => ({
      name: String(row.name),
      currentClass: Number(row.current_class),
      level: Number(row.level),
      rebirths: Number(row.rebirths),
      gained: Number(row.gained),
    }));
  } catch (error) {
    console.error("powergamers query failed", error);
    return null;
  }
}

export type Killer = { name: string; currentClass: number; level: number; kills: number };
export const killerPeriods = ["day", "week", "month", "all"] as const;
export type KillerPeriod = (typeof killerPeriods)[number];
const periodSeconds: Record<KillerPeriod, number> = { day: 86_400, week: 7 * 86_400, month: 30 * 86_400, all: 0 };

/** Players killed by players in the period. */
export async function getKillers(period: KillerPeriod, profession: Profession | null): Promise<Killer[] | null> {
  "use cache";
  cacheLife("minutes");
  const range = profession ? professionRange[profession] : [0, 1_000];
  try {
    const { rows } = await pool.query(
      `SELECT k.name, k.current_class, k.level, COUNT(*)::int AS kills
         FROM character_deaths d JOIN characters k ON k.character_id = d.killer_id
        WHERE ($1::bigint = 0 OR d.died_at >= EXTRACT(EPOCH FROM now())::bigint - $1::bigint)
          AND k.current_class BETWEEN $2 AND $3
        GROUP BY k.character_id
        ORDER BY kills DESC, k.name LIMIT 50`,
      [periodSeconds[period], range[0], range[1]],
    );
    return rows.map((row) => ({
      name: String(row.name),
      currentClass: Number(row.current_class),
      level: Number(row.level),
      kills: Number(row.kills),
    }));
  } catch (error) {
    console.error("killers query failed", error);
    return null;
  }
}

/** The latest deaths on the server, for the Killers page. */
export async function getLatestDeaths(): Promise<(Death & { victim: string })[] | null> {
  "use cache";
  cacheLife("minutes");
  try {
    const { rows } = await pool.query(
      `SELECT d.died_at, d.level, d.killer_name AS name, k.name AS character, d.killer_level AS other_level, v.name AS victim
         FROM character_deaths d
         JOIN characters v ON v.character_id = d.character_id
         LEFT JOIN characters k ON k.character_id = d.killer_id
        ORDER BY d.died_at DESC LIMIT 20`,
    );
    return rows.map((row) => ({ ...death(row), victim: String(row.victim) }));
  } catch (error) {
    console.error("deaths query failed", error);
    return null;
  }
}

export type GuildSummary = {
  name: string;
  leader: string;
  members: number;
  averageLevel: number;
  silver: number;
  kills: number;
  createdAt: number;
};

export async function getGuilds(): Promise<GuildSummary[] | null> {
  "use cache";
  cacheLife("minutes");
  try {
    const { rows } = await pool.query(
      `SELECT g.name, l.name AS leader, g.silver, g.created_at,
              COUNT(m.character_id)::int AS members, COALESCE(ROUND(AVG(c.level)), 0)::int AS average_level,
              (SELECT COUNT(*)::int FROM character_deaths d JOIN guild_members km ON km.character_id = d.killer_id
                WHERE km.guild_id = g.guild_id) AS kills
         FROM guilds g
         JOIN characters l ON l.character_id = g.leader_id
         LEFT JOIN guild_members m ON m.guild_id = g.guild_id
         LEFT JOIN characters c ON c.character_id = m.character_id
        GROUP BY g.guild_id, l.name
        ORDER BY members DESC, g.silver DESC, g.name`,
    );
    return rows.map((row) => ({
      name: String(row.name),
      leader: String(row.leader),
      members: Number(row.members),
      averageLevel: Number(row.average_level),
      silver: Number(row.silver),
      kills: Number(row.kills),
      createdAt: Number(row.created_at),
    }));
  } catch (error) {
    console.error("guilds query failed", error);
    return null;
  }
}

export type GuildMember = { name: string; currentClass: number; level: number; role: number; online: boolean; joinedAt: number };
export type Guild = GuildSummary & { level: number; roster: GuildMember[] };

export async function getGuild(name: string): Promise<Guild | null | undefined> {
  "use cache";
  cacheLife("minutes");
  try {
    const { rows } = await pool.query(
      `SELECT g.guild_id, g.name, g.level, g.silver, g.created_at, l.name AS leader
         FROM guilds g JOIN characters l ON l.character_id = g.leader_id
        WHERE lower(g.name) = lower($1)`,
      [name],
    );
    const guild = rows[0];
    if (!guild) return null;
    const [members, kills] = await Promise.all([
      pool.query(
        `SELECT c.name, c.current_class, c.level, m.role, m.joined_at,
                c.last_seen > 0 AND EXTRACT(EPOCH FROM now())::bigint - c.last_seen <= $2 AS online
           FROM guild_members m JOIN characters c ON c.character_id = m.character_id
          WHERE m.guild_id = $1 ORDER BY m.role DESC, c.level DESC, c.name`,
        [guild.guild_id, ONLINE_WITHIN],
      ),
      pool.query(
        `SELECT COUNT(*)::int AS kills FROM character_deaths d JOIN guild_members km ON km.character_id = d.killer_id
          WHERE km.guild_id = $1`,
        [guild.guild_id],
      ),
    ]);
    const roster: GuildMember[] = members.rows.map((row) => ({
      name: String(row.name),
      currentClass: Number(row.current_class),
      level: Number(row.level),
      role: Number(row.role),
      online: Boolean(row.online),
      joinedAt: Number(row.joined_at),
    }));
    return {
      name: String(guild.name),
      leader: String(guild.leader),
      level: Number(guild.level),
      silver: Number(guild.silver),
      createdAt: Number(guild.created_at),
      members: roster.length,
      averageLevel: roster.length ? Math.round(roster.reduce((sum, member) => sum + member.level, 0) / roster.length) : 0,
      kills: Number(kills.rows[0].kills),
      roster,
    };
  } catch (error) {
    console.error("guild query failed", error);
    return undefined;
  }
}
