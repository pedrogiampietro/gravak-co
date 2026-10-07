// What the server's numbers mean, shared by the pages (no database here).

export type Profession = "trojan" | "warrior" | "archer" | "taoist";

export const professions: readonly Profession[] = ["trojan", "warrior", "archer", "taoist"];

/** `characters.current_class` to its profession (the server's `Profession::holds`). */
export function professionOf(currentClass: number): Profession | null {
  if (currentClass >= 10 && currentClass <= 15) return "trojan";
  if (currentClass >= 20 && currentClass <= 25) return "warrior";
  if (currentClass >= 40 && currentClass <= 45) return "archer";
  if (currentClass >= 100 && currentClass <= 145) return "taoist";
  return null;
}

/** The class ranges a profession filter keeps, for SQL. */
export const professionRange: Record<Profession, [number, number]> = {
  trojan: [10, 15],
  warrior: [20, 25],
  archer: [40, 45],
  taoist: [100, 145],
};

export type NobilityRank = "king" | "prince" | "duke" | "earl" | "baron" | "knight" | null;

/** The server's `nobility::rank_at`: seats for the first 50, thresholds after. */
export function nobilityRank(position: number, donation: number): NobilityRank {
  if (donation > 0) {
    if (position <= 3) return "king";
    if (position <= 15) return "prince";
    if (position <= 50) return "duke";
  }
  if (donation >= 200_000_000) return "earl";
  if (donation >= 100_000_000) return "baron";
  if (donation >= 30_000_000) return "knight";
  return null;
}

export const nobilityNames: Record<Exclude<NobilityRank, null>, string> = {
  king: "King",
  prince: "Prince",
  duke: "Duke",
  earl: "Earl",
  baron: "Baron",
  knight: "Knight",
};
