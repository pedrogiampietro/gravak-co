//! What the item menu's forge actions cost, to show in their panels: the
//! server's `forge.rs` rules (the NPCs' own: MagicArtisan, WeaponMaster,
//! WuxingOven, GodlyArtisan), with tests on the same numbers as the
//! server's. The server decides; this only tells the player beforehand.

use crate::network::OwnedItem;

pub const DRAGONBALL: u32 = 1_088_000;
pub const METEOR: u32 = 1_088_001;
pub const DBSCROLL: u32 = 720_028;
pub const TOUGH_DRILL: u32 = 1_200_005;
pub const STAR_DRILL: u32 = 1_200_006;

/// What a forge action will take.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ForgePrice {
    /// Nothing more can be done.
    #[default]
    Done,
    /// `count` bag items of `item_type`; `result` is the new type or plus.
    Pay { item_type: u32, count: u32, result: u32 },
    /// Two bag items of +`minor` melted in, the item becomes +`result`.
    Pair { minor: u8, result: u8 },
    /// Composing below +9 needs two items of the same +n.
    NeedPair,
    /// The second socket of gear: 7 StarDrills, or a ToughDrill's luck.
    Drill,
}

fn quality(item_type: u32) -> u32 {
    item_type % 10
}

fn equipment_level(item_type: u32) -> u32 {
    item_type % 1_000 / 10
}

fn armour_like(item_type: u32) -> bool {
    let sort = item_type % 100_000 / 10_000;
    (item_type / 100_000 == 1 && matches!(sort, 1 | 3 | 4)) || item_type / 1_000 == 900
}

pub fn next_quality(item_type: u32, exists: impl Fn(u32) -> bool) -> Option<u32> {
    let quality = quality(item_type);
    if quality < 3 || quality == 9 {
        return None;
    }
    let base = if quality < 5 { item_type / 10 * 10 + 5 } else { item_type };
    Some(base + 1).filter(|next| exists(*next))
}

fn quality_chance(item_type: u32, requirement: u32) -> u32 {
    let quality = quality(item_type);
    if quality == 9 || quality < 3 {
        return 0;
    }
    let mut chance = match quality.max(5) {
        6 => 50,
        7 => 33,
        8 => 20,
        _ => 100,
    };
    if requirement > 70 {
        chance = chance * 100_u32.saturating_sub(requirement - 70) / 100;
    }
    chance.max(1)
}

pub fn quality_cost(item_type: u32, requirement: u32) -> u32 {
    1 + 100 / quality_chance(item_type, requirement).max(1)
}

pub fn next_level(item_type: u32, exists: impl Fn(u32) -> bool) -> Option<u32> {
    if quality(item_type) < 3 {
        return None;
    }
    (1..10)
        .map(|step| item_type + 10 * step)
        .find(|candidate| exists(*candidate))
        .filter(|next| next / 1_000 == item_type / 1_000)
}

fn level_chance(item_type: u32) -> u32 {
    let level = equipment_level(item_type);
    let chance: u32 = if armour_like(item_type) {
        match level {
            5 => 50,
            6 => 40,
            7 => 30,
            8 | 9 => 20,
            _ => 500,
        }
    } else {
        match level {
            11..=22 => 95 - (level - 11) * 5,
            _ => 500,
        }
    };
    let chance = match quality(item_type) {
        6 => chance * 90 / 100,
        7 => chance * 70 / 100,
        8 => chance * 30 / 100,
        9 => chance * 10 / 100,
        _ => chance,
    };
    chance.clamp(2, 100)
}

pub fn level_cost(item_type: u32) -> u32 {
    (1 + 100 / level_chance(item_type)).min(40)
}

fn composed_plus(main: u8, minor: u8) -> Option<u8> {
    if main >= 9 || minor == 0 {
        return None;
    }
    Some(if minor > main { minor.min(9) } else { main + 1 })
}

pub fn best_pair(plus: u8, pairs: impl IntoIterator<Item = u8>) -> Option<(u8, u8)> {
    pairs
        .into_iter()
        .filter_map(|minor| Some((minor, composed_plus(plus, minor)?)))
        .max_by_key(|(minor, result)| (*result, std::cmp::Reverse(*minor)))
}

/// QUALITY: MagicArtisan's next quality and DragonBalls.
pub fn quality_price(item_type: u32, requirement: u32, exists: impl Fn(u32) -> bool) -> ForgePrice {
    match next_quality(item_type, exists) {
        Some(next) => ForgePrice::Pay {
            item_type: DRAGONBALL,
            count: quality_cost(item_type, requirement),
            result: next,
        },
        None => ForgePrice::Done,
    }
}

/// LEVEL: Meteors at MagicArtisan below level 120, a DragonBall at
/// WeaponMaster above.
pub fn level_price(item_type: u32, requirement: u32, exists: impl Fn(u32) -> bool) -> ForgePrice {
    match next_level(item_type, exists) {
        Some(next) if requirement < 120 => ForgePrice::Pay {
            item_type: METEOR,
            count: level_cost(item_type),
            result: next,
        },
        Some(next) => ForgePrice::Pay {
            item_type: DRAGONBALL,
            count: 1,
            result: next,
        },
        None => ForgePrice::Done,
    }
}

/// UPGRADE: WuxingOven's next plus for `item`, from what the bag holds.
pub fn plus_price(item: OwnedItem, bag: &[OwnedItem]) -> ForgePrice {
    match item.plus {
        12.. => ForgePrice::Done,
        9 => ForgePrice::Pay { item_type: DRAGONBALL, count: 12, result: 10 },
        10 => ForgePrice::Pay { item_type: DRAGONBALL, count: 25, result: 11 },
        11 => ForgePrice::Pay { item_type: DBSCROLL, count: 4, result: 12 },
        plus => {
            let mut counts = std::collections::BTreeMap::<u8, usize>::new();
            for owned in bag.iter().filter(|owned| owned.position < 40 && owned.uid != item.uid) {
                if (1..=9).contains(&owned.plus) {
                    *counts.entry(owned.plus).or_default() += 1;
                }
            }
            let pairs = counts.into_iter().filter(|(_, count)| *count >= 2).map(|(plus, _)| plus);
            match best_pair(plus, pairs) {
                Some((minor, result)) => ForgePrice::Pair { minor, result },
                None => ForgePrice::NeedPair,
            }
        }
    }
}

fn is_weapon(item_type: u32) -> bool {
    matches!(item_type / 100_000, 4 | 5)
}

/// GEM SOCKET's OPEN: GodlyArtisan's price for the next socket.
pub fn socket_price(item: OwnedItem, bag: &[OwnedItem]) -> ForgePrice {
    let first = item.gem_one != 0;
    let second = item.gem_two != 0;
    if first && second {
        return ForgePrice::Done;
    }
    let pay = |item_type, count| ForgePrice::Pay { item_type, count, result: 0 };
    match (is_weapon(item.item_type), first) {
        (true, false) => pay(DRAGONBALL, 1),
        (true, true) => pay(DRAGONBALL, 5),
        (false, false) => pay(DRAGONBALL, 12),
        (false, true) if count(bag, STAR_DRILL, item.uid) >= 7 => pay(STAR_DRILL, 7),
        (false, true) => ForgePrice::Drill,
    }
}

/// Bag items of `item_type`, not counting `except`.
pub fn count(bag: &[OwnedItem], item_type: u32, except: u32) -> usize {
    bag.iter()
        .filter(|owned| owned.position < 40 && owned.item_type == item_type && owned.uid != except)
        .count()
}

pub fn catalyst_name(item_type: u32) -> &'static str {
    match item_type {
        DRAGONBALL => "DragonBall",
        METEOR => "Meteor",
        DBSCROLL => "DBScroll",
        STAR_DRILL => "StarDrill",
        TOUGH_DRILL => "ToughDrill",
        _ => "item",
    }
}

impl ForgePrice {
    /// Whether the bag holds what it takes (the item `except` aside).
    pub fn affordable(self, bag: &[OwnedItem], except: u32) -> bool {
        match self {
            Self::Pay { item_type, count: needed, .. } => count(bag, item_type, except) >= needed as usize,
            Self::Pair { .. } => true,
            Self::Drill => count(bag, TOUGH_DRILL, except) >= 1,
            Self::Done | Self::NeedPair => false,
        }
    }

    /// The cost as a panel says it, with what the bag holds.
    pub fn describe(self, bag: &[OwnedItem], except: u32) -> String {
        match self {
            Self::Pay { item_type, count: needed, .. } => format!(
                "Cost: {needed} {}{}  (you have {})",
                catalyst_name(item_type),
                if needed == 1 { "" } else { "s" },
                count(bag, item_type, except)
            ),
            Self::Pair { minor, result } => format!("Melts two +{minor} items from your bag: +{result}"),
            Self::NeedPair => "Needs two items of the same +n (or +n Stones) in your bag".to_owned(),
            Self::Drill => format!(
                "Cost: 7 StarDrills, or a ToughDrill (12%)  (you have {})",
                count(bag, TOUGH_DRILL, except)
            ),
            Self::Done => "It cannot be upgraded further".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(uid: u32, item_type: u32, plus: u8) -> OwnedItem {
        OwnedItem { uid, item_type, plus, position: uid as u8, ..OwnedItem::default() }
    }

    /// The server's `forge::tests` numbers.
    #[test]
    fn the_panels_ask_what_the_npcs_ask() {
        let all = |_| true;
        assert_eq!(quality_price(410_205, 105, all), ForgePrice::Pay { item_type: DRAGONBALL, count: 2, result: 410_206 });
        assert_eq!(quality_price(410_208, 105, all), ForgePrice::Pay { item_type: DRAGONBALL, count: 8, result: 410_209 });
        assert_eq!(quality_price(410_209, 105, all), ForgePrice::Done);
        assert_eq!(level_cost(410_115), 2);
        assert_eq!(level_cost(410_208), 7);
        assert_eq!(level_cost(410_209), 21);
        assert!(matches!(level_price(410_235, 120, all), ForgePrice::Pay { item_type: DRAGONBALL, count: 1, .. }));
        let blade = item(50, 410_205, 9);
        assert_eq!(plus_price(blade, &[]), ForgePrice::Pay { item_type: DRAGONBALL, count: 12, result: 10 });
        let stones = [item(1, 730_008, 8), item(2, 730_008, 8), item(3, 730_001, 1), item(4, 730_001, 1)];
        assert_eq!(plus_price(item(50, 410_205, 0), &stones), ForgePrice::Pair { minor: 8, result: 8 });
        assert_eq!(plus_price(item(50, 410_205, 5), &stones[2..]), ForgePrice::Pair { minor: 1, result: 6 });
        assert_eq!(plus_price(item(50, 410_205, 3), &stones[..1]), ForgePrice::NeedPair);
        assert_eq!(socket_price(item(50, 410_205, 0), &[]), ForgePrice::Pay { item_type: DRAGONBALL, count: 1, result: 0 });
        let mut socketed = item(50, 130_037, 0);
        assert_eq!(socket_price(socketed, &[]), ForgePrice::Pay { item_type: DRAGONBALL, count: 12, result: 0 });
        socketed.gem_one = 255;
        assert_eq!(socket_price(socketed, &[]), ForgePrice::Drill);
    }
}
