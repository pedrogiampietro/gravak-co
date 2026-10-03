use num_enum::{FromPrimitive, IntoPrimitive};
use serde::Deserialize;
use tq_network::{Actor, PacketID, PacketProcess};

use crate::entities::NpcKind;
use crate::packets::{MsgTalk, MsgTaskDialog};

#[derive(Default, Debug, Clone, Copy, FromPrimitive, IntoPrimitive)]
#[repr(u16)]
pub enum NpcActionKind {
    #[default]
    Activate = 0,
    AddNpc = 1,
    LeaveMap = 2,
    DeleteNpc = 3,
    ChangePosition = 4,
    LayNpc = 5,
    CancelInteraction = 255,
}

/// This packet is used to interact with a NPC and contains multiple
/// DialogAction types that are used to determine the type of interaction.
#[derive(Debug, Deserialize, Clone, PacketID)]
#[packet(id = 2031)]
pub struct MsgNpc {
    npc_id: u32,
    data: u32,
    action: u16,
    kind: u16,
}

#[async_trait::async_trait]
impl PacketProcess for MsgNpc {
    type ActorState = crate::ActorState;
    type Error = crate::Error;
    type State = crate::State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        tracing::debug!(
            npc_id = self.npc_id,
            data = self.data,
            action = ?NpcActionKind::from(self.action),
            kind = ?NpcKind::from(self.kind as u8),
            "MsgNpc received"
        );
        let me = actor.entity();
        let character = me.as_character().ok_or(crate::Error::CharacterNotFound)?;
        let mymap = state.try_map(me.basic().map_id())?;
        let npc = match mymap.npc(self.npc_id) {
            Some(npc) => npc,
            None => {
                actor
                    .send(MsgTalk::from_system(
                        me.id(),
                        super::TalkChannel::System,
                        format!("NPC {} not found", self.npc_id),
                    ))
                    .await?;
                return Ok(());
            },
        };
        let my_loc = me.basic().location();
        let npc_loc = npc.entity().location();
        let in_screen = tq_math::in_screen(my_loc.into(), npc_loc.into());
        if !in_screen {
            tracing::warn!(
                target: "world",
                character = %me.basic().name(),
                account = actor.id(),
                npc = npc.id(),
                "NPC used from out of sight; ignored"
            );
            return Ok(());
        }
        let npc_id = npc.id();
        // The pole and the gates only talk to the winning guild.
        if crate::guild_war::is_war_structure(npc_id) && crate::guild_war::opening(state, npc_id, character).is_none() {
            return Ok(());
        }
        let npc_name = npc.entity().name();
        let shop_items = state
            .shop_catalog()
            .offer(npc_id)
            .map_or(0, |offer| offer.item_ids.len());
        let (text, options, avatar) = crate::teleports::opening(npc_id)
            .map(|(lines, options, face)| (lines.concat(), options, face))
            .or_else(|| crate::trainers::opening(npc_id, character))
            .or_else(|| crate::market_npcs::opening(npc_id, character))
            .or_else(|| crate::quest_npcs::opening(npc_id, character))
            .or_else(|| crate::guild_war::opening(state, npc_id, character))
            .or_else(|| crate::city_npcs::opening(npc_id, character))
            .or_else(|| crate::offline_tg::opening(npc_id))
            .or_else(|| crate::artisans::opening(npc_id))
            .or_else(|| teleport_script(npc_id))
            .unwrap_or_else(|| npc_script(npc_name, npc.kind(), shop_items));
        let closes_itself = options.iter().any(|(option_id, _)| *option_id == 255);
        let mut builder = MsgTaskDialog::builder().text(text);
        for (option_id, option_text) in options {
            builder = builder.with_option(option_id, option_text);
        }
        if !closes_itself {
            builder = builder.with_option(255, "Leave.");
        }
        let dialog = builder.and().with_avatar(avatar).task_id(npc_id).build();
        actor.send_all(dialog).await?;
        Ok(())
    }
}

/// Select the first interaction for an NPC.  `kind` describes the service
/// category, while the name identifies the quest or town NPC within that
/// category.  Keeping this dispatch in one place prevents every unknown NPC
/// from falling through to the same placeholder conversation.
/// Dialog text, `(option id, label)` pairs and avatar of an NPC.
type NpcDialog = (String, Vec<(u8, String)>, u16);

fn npc_script(name: &str, kind: NpcKind, shop_items: usize) -> NpcDialog {
    let lower = name.to_ascii_lowercase();
    if kind == NpcKind::ShopKeeper {
        if lower.contains("pharmac") {
            return (
                format!("{name}: medicines and antidotes for every journey. {shop_items} items are available."),
                vec![(1, "Buy medicine.".into()), (2, "Sell items.".into())],
                30,
            );
        }
        if lower.contains("blacksmith") || lower.contains("armorer") || lower.contains("weapon") {
            return (
                format!("{name}: I repair and sell dependable equipment. {shop_items} items are available."),
                vec![
                    (1, "Buy equipment.".into()),
                    (2, "Sell equipment.".into()),
                    (3, "Repair equipment.".into()),
                ],
                40,
            );
        }
        if lower.contains("barber") {
            return (
                format!("{name}: a new style can make any hero feel ready for adventure."),
                vec![(1, "Change my appearance.".into())],
                50,
            );
        }
        return (
            format!("Welcome to {name}'s shop. {shop_items} items are available."),
            vec![(1, "Buy items.".into()), (2, "Sell items.".into())],
            47,
        );
    }
    if kind == NpcKind::Storage || lower.contains("warehouse") || lower.contains("whsguardian") {
        return (
            format!("{name}: I will keep your belongings safe while you travel."),
            vec![(1, "Open warehouse.".into())],
            47,
        );
    }
    if lower.contains("jewel") {
        return (
            format!("{name}: I can compose gems into higher qualities."),
            vec![(1, "Compose normal gems.".into()), (2, "Compose refined gems.".into())],
            47,
        );
    }
    if kind == NpcKind::Forge || lower.contains("artisan") || lower.contains("master") {
        return (
            format!("{name}: I can improve the quality of your equipment."),
            vec![
                (1, "Improve item level with a Meteor.".into()),
                (2, "Improve item quality with a DragonBall.".into()),
            ],
            47,
        );
    }
    if kind == NpcKind::Face || lower.contains("barber") {
        return (
            format!("{name}: your appearance can be changed here."),
            vec![(1, "Show appearance options.".into())],
            50,
        );
    }
    if lower == "oldgeneralyang" {
        return (
            "I train the young heroes of Twin City.".into(),
            vec![
                (1, "How can I become stronger?".into()),
                (2, "I will return later.".into()),
                (3, "Teach me a skill.".into()),
                (4, "Do you have a task for me?".into()),
            ],
            47,
        );
    }
    if lower == "warden" {
        return (
            format!("{name}: you will leave this place once your sentence has been served."),
            Vec::new(),
            47,
        );
    }
    if lower.contains("guild") {
        return (
            format!("{name}: guilds give adventurers a place to work together."),
            vec![(1, "Tell me about guilds.".into()), (2, "I will return later.".into())],
            47,
        );
    }
    if lower.contains("trainer") || lower.contains("star") || lower.contains("god") || lower.contains("general") {
        return (
            format!("{name}: practice every day and your strength will follow."),
            vec![
                (1, "How can I become stronger?".into()),
                (2, "Show me your advice.".into()),
                (3, "Teach me a skill.".into()),
            ],
            47,
        );
    }
    match name {
        "KnowItAll" => (
            "Knowledge is the first step on every adventure.".into(),
            vec![
                (1, "Tell me about Twin City.".into()),
                (2, "Do you know any secrets?".into()),
                (3, "Teleport me to Twin City.".into()),
            ],
            47,
        ),
        "Mr.Nosy" => (
            "Psst... I hear everything that happens around here.".into(),
            vec![(1, "What did you hear?".into()), (2, "Never mind.".into())],
            47,
        ),
        "OldGeneralYang" => (
            "I train the young heroes of Twin City.".into(),
            vec![
                (1, "How can I become stronger?".into()),
                (2, "I will return later.".into()),
                (3, "Teach me a skill.".into()),
                (4, "Do you have a task for me?".into()),
            ],
            47,
        ),
        _ if lower.contains("scarecrow")
            || lower.contains("stake")
            || lower.contains("shopflag")
            || lower.contains("furniture") =>
        {
            (format!("{name} is part of the training grounds."), Vec::new(), 47)
        },
        _ => (
            format!("{name}: welcome to Twin City. Ask around to discover what I can do."),
            vec![
                (1, "What can you tell me?".into()),
                (2, "Tell me about this city.".into()),
            ],
            47,
        ),
    }
}

/// Dialog of the NPCs that run a teleport service. The options map to the
/// destinations resolved by [`crate::teleports::resolve`].
fn teleport_script(npc_id: u32) -> Option<NpcDialog> {
    use crate::teleports;
    if let Some(conductress) = teleports::conductress(npc_id) {
        let options = conductress
            .destinations
            .iter()
            .zip(1u8..)
            .map(|(destination, option_id)| {
                (
                    option_id,
                    format!("{} ({} silver).", destination.name, teleports::CONDUCTRESS_FEE),
                )
            })
            .collect();
        return Some((
            format!(
                "Conductress: welcome to {}. For {} silver I can take you to the city gates or to the Market.",
                conductress.city,
                teleports::CONDUCTRESS_FEE
            ),
            options,
            47,
        ));
    }
    if teleports::is_market_controller(npc_id) {
        return Some((
            "Mark.Controller: when you are done trading, I will send you back to where you came from.".into(),
            vec![(teleports::LEAVE_MARKET_OPTION, "Leave the Market.".into())],
            47,
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{npc_script, teleport_script};
    use crate::entities::NpcKind;

    #[test]
    fn trainer_dialog_exposes_skill_training_option() {
        let (_, options, _) = npc_script("OldGeneralYang", NpcKind::Task, 0);
        assert!(options.iter().any(|(id, _)| *id == 3));
        assert!(options.iter().any(|(id, _)| *id == 4));
    }

    #[test]
    fn blacksmith_dialog_exposes_repair_option() {
        let (_, options, _) = npc_script("Blacksmith", NpcKind::ShopKeeper, 4);
        assert!(options.iter().any(|(id, _)| *id == 3));
    }

    #[test]
    fn artisan_dialog_exposes_upgrade_option() {
        let (_, options, _) = npc_script("ArtisanWind", NpcKind::Forge, 0);
        assert!(options.iter().any(|(id, _)| *id == 1));
        assert!(options.iter().any(|(id, _)| *id == 2));
    }

    #[test]
    fn jeweler_dialog_exposes_gem_tiers() {
        let (_, options, _) = npc_script("JewelerLau", NpcKind::Task, 0);
        assert!(options.iter().any(|(id, _)| *id == 1));
        assert!(options.iter().any(|(id, _)| *id == 2));
    }

    #[test]
    fn conductress_dialog_lists_every_destination_with_its_fee() {
        let Some((_, options, _)) = teleport_script(10050) else {
            panic!("Twin City conductress must have a dialog");
        };
        assert_eq!(
            options.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5, 6]
        );
        assert!(options.iter().all(|(_, text)| text.contains("100 silver")));
    }

    #[test]
    fn prison_wardens_do_not_offer_teleports() {
        assert!(teleport_script(42).is_none());
        let (_, options, _) = npc_script("Warden", NpcKind::Task, 0);
        assert!(options.is_empty());
    }
}
