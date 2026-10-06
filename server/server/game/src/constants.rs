#![allow(unused)]

pub const SYSTEM: &str = "SYSTEM";
pub const ALL_USERS: &str = "ALLUSERS";
pub const ANSWER_OK: &str = "ANSWER_OK";
pub const NEW_ROLE: &str = "NEW_ROLE";

pub const MAX_TXT_LEN: usize = 250;

pub const HAIR_STYLES: [i16; 12] = [10, 11, 13, 14, 15, 24, 30, 35, 37, 38, 39, 40];

/// Every hair style of 5065 (`ini/armet.ini`, the same for each body): what
/// the creation screen offers. `HAIR_STYLES` are the ones drawn at random.
pub const ALL_HAIR_STYLES: [u16; 35] = [
    10, 11, 12, 13, 14, 15, 16, 17, 21, 22, 23, 24, 25, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43,
    44, 45, 46, 47, 48, 49, 50, 51,
];
/// Hair colours: the hundreds of a hair value.
pub const HAIR_COLOURS: std::ops::RangeInclusive<u16> = 3..=9;
/// The faces a new character may pick, by sex (`data/PlayerFace`).
pub const MALE_AVATARS: std::ops::RangeInclusive<u16> = 1..=102;
pub const FEMALE_AVATARS: std::ops::RangeInclusive<u16> = 201..=290;

pub const WALK_XCOORDS: [i8; 8] = [0, -1, -1, -1, 0, 1, 1, 1];
pub const WALK_YCOORDS: [i8; 8] = [1, 1, 0, -1, -1, -1, 0, 1];

pub const NPC_ID_MIN: u32 = 1;
pub const DYN_NPC_ID_MIN: u32 = 100001;
pub const DYN_NPC_ID_MAX: u32 = 199999;
pub const MONSTER_ID_MIN: u32 = 400001;
pub const MONSTER_ID_MAX: u32 = 499999;
/// Monsters a game master creates (`$monster`): below the summoned pets,
/// above every spawn of the map data.
pub const SUMMONED_ID_MIN: u32 = 490000;
pub const SUMMONED_ID_MAX: u32 = 494999;
pub const PET_ID_MIN: u32 = 500001;
pub const PET_ID_MAX: u32 = 599999;
pub const NPC_ID_MAX: u32 = 700000;
pub const CALL_PET_ID_MIN: u32 = 700001;
pub const CALL_PET_ID_MAX: u32 = 799999;
pub const CHARACTER_ID_MIN: u32 = 1000000;
pub const CHARACTER_ID_MAX: u32 = 10000000;

pub const fn is_npc(id: u32) -> bool {
    id >= NPC_ID_MIN && id <= NPC_ID_MAX
}

pub const fn is_terrain_npc(id: u32) -> bool {
    id >= DYN_NPC_ID_MIN && id <= DYN_NPC_ID_MAX
}

/// Whether `id` is a monster created by a game master: it never respawns.
pub const fn is_summoned(id: u32) -> bool {
    id >= SUMMONED_ID_MIN && id <= SUMMONED_ID_MAX
}

pub const fn is_monster(id: u32) -> bool {
    id >= MONSTER_ID_MIN && id <= MONSTER_ID_MAX
}

pub const fn is_pet(id: u32) -> bool {
    id >= PET_ID_MIN && id <= PET_ID_MAX
}

pub const fn is_call_pet(id: u32) -> bool {
    id >= CALL_PET_ID_MIN && id <= CALL_PET_ID_MAX
}

pub const fn is_character(id: u32) -> bool {
    id >= CHARACTER_ID_MIN && id <= CHARACTER_ID_MAX
}
