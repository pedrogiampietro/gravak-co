//! The super gems' shine (Redux `DisplaySuperGemEffects`): each blow or
//! attack skill of a hero wearing super gems shows, (2 + their count) times
//! in a hundred, the effect of one of them picked at random to those around:
//! the golden dragon of a Dragon gem, the phoenix of a Phoenix gem...

use crate::entities::Character;
use crate::Error;

/// Each gem family's effect (`3DEffect.ini`), by the code's tens: Phoenix,
/// Dragon, Fury, Rainbow, Kylin, Violet, Moon, Tortoise.
const EFFECTS: [&str; 8] = ["phoenix", "goldendragon", "lounder1", "rainbow", "goldenkylin", "purpleray", "moon", "recovery"];

/// The super gems among socket codes (family * 10 + quality, 3 is super).
fn super_gems(codes: impl IntoIterator<Item = u8>) -> Vec<u8> {
    codes
        .into_iter()
        .filter(|code| *code != 0 && *code != 255 && code % 10 == 3 && usize::from(code / 10) < EFFECTS.len())
        .collect()
}

/// The effect a blow shows, if any: `chance_roll` decides whether, and
/// `pick_roll` which super gem.
pub fn effect_for(codes: impl IntoIterator<Item = u8>, chance_roll: u32, pick_roll: u32) -> Option<&'static str> {
    let gems = super_gems(codes);
    if gems.is_empty() || chance_roll % 100 >= 2 + gems.len() as u32 {
        return None;
    }
    let gem = gems[pick_roll as usize % gems.len()];
    Some(EFFECTS[usize::from(gem / 10)])
}

/// A blow or attack skill of `character`: maybe their super gems shine.
pub async fn on_attack(character: &Character) -> Result<(), Error> {
    let codes = character
        .equipment_items()
        .into_iter()
        .filter(|item| item.durability > 0)
        .flat_map(|item| [item.gem_one, item.gem_two]);
    let Some(effect) = effect_for(codes, crate::combat::next_roll(), crate::combat::next_roll()) else {
        return Ok(());
    };
    let packet = crate::packets::MsgName::role_effect(character.id(), effect);
    character.owner().send(packet.clone()).await?;
    if let Ok(screen) = character.try_screen() {
        screen.send_message(packet).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_super_gems_shine_and_more_of_them_shine_more() {
        // Normal and refined gems never shine.
        assert_eq!(effect_for([11, 12, 255], 0, 0), None);
        // A Super Dragon gem: 3 times in a hundred.
        assert_eq!(effect_for([13], 2, 0), Some("goldendragon"));
        assert_eq!(effect_for([13], 3, 0), None);
        // Two super gems: 4 in a hundred, one of the two.
        assert_eq!(effect_for([13, 3], 3, 1), Some("phoenix"));
        assert_eq!(effect_for([13, 3], 4, 1), None);
        assert_eq!(effect_for([73], 0, 0), Some("recovery"));
    }
}
