//! Potions and arrows stack in the bag: one slot holds many of a kind.
//!
//! A stack's amount is its durability, as an arrow pack's arrows always
//! were; its most durability is one unit (1 for a potion, the pack's
//! arrows for arrows), so a stack holds `durability / max_durability`
//! units, at most `STACK_UNITS` of them. Selling (a third of the price,
//! times the durability over the most), trading and storing need nothing
//! else: the whole stack goes with its amount.

use crate::shop::ShopCatalog;

/// The units of a kind one slot holds.
pub const POTION_STACK: u16 = 30;
pub const ARROW_PACKS: u16 = 5;

/// A life or mana potion the shops sell.
pub fn is_potion(item_type: u32) -> bool {
    (1_000_000..1_003_000).contains(&item_type)
}

pub fn stackable(item_type: u32) -> bool {
    is_potion(item_type) || ShopCatalog::is_arrow(item_type)
}

/// The durability an item of `item_type` comes with: a potion whose most is
/// not one is a single potion as the catalog makes it (1/1); a stack (most
/// 1) keeps its amount.
pub fn born(item_type: u32, durability: u16, max_durability: u16) -> (u16, u16) {
    if is_potion(item_type) && max_durability != 1 { (1, 1) } else { (durability, max_durability) }
}

/// The most durability a stack of `item_type` reaches.
pub fn capacity(item_type: u32, max_durability: u16) -> u16 {
    let units = if is_potion(item_type) { POTION_STACK } else { ARROW_PACKS };
    max_durability.max(1).saturating_mul(units)
}

/// How much of `amount` a stack at `durability` still takes.
pub fn room(item_type: u32, durability: u16, max_durability: u16) -> u16 {
    capacity(item_type, max_durability).saturating_sub(durability)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn potions_and_arrows_stack_by_their_units() {
        assert!(stackable(1_000_030) && stackable(1_050_002));
        assert!(!stackable(410_301) && !stackable(1_088_000), "a blade, a DragonBall");
        // A potion is one, whatever the catalog said.
        assert_eq!(born(1_002_020, 100, 100), (1, 1));
        assert_eq!(born(1_002_020, 12, 1), (12, 1), "a stack keeps its amount");
        assert_eq!(born(1_050_002, 500, 500), (500, 500));
        // Thirty potions, five packs of arrows.
        assert_eq!(capacity(1_002_020, 1), 30);
        assert_eq!(capacity(1_050_002, 500), 2_500);
        assert_eq!(room(1_002_020, 28, 1), 2);
        assert_eq!(room(1_050_002, 2_500, 500), 0);
    }
}
