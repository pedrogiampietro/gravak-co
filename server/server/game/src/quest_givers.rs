//! The NPCs who give quest chains (`quests.rs`): Old General Yang in Twin
//! City, and one in each of the other cities. Asking for a task starts the
//! next quest, shows the one under way, or takes a delivery's items.

use tq_network::Actor;

use crate::entities::Character;
use crate::packets::{MsgItemInfo, MsgTalk, MsgUserInfo, TalkChannel};
use crate::quests::{Offer, TurnIn};
use crate::{ActorState, Error, State};

/// A quest giver: their NPC, the dialog option that asks for a task, their
/// name in replies, the greeting, and what they say when all is done.
pub struct Giver {
    pub npc_id: u32,
    pub option: u8,
    pub name: &'static str,
    greeting: &'static str,
    all_done: &'static str,
}

pub const GIVERS: [Giver; 7] = [
    // Old General Yang's own dialog (`msg_npc.rs`) lists the option.
    Giver {
        npc_id: 425,
        option: 4,
        name: "Old General Yang",
        greeting: "",
        all_done: "you have done all I could ask of you. Twin City is grateful.",
    },
    Giver {
        npc_id: 30_081,
        option: 1,
        name: "CoachLin",
        greeting: "Phoenix Castle needs brave hands. The roads and hills are full of trouble.",
        all_done: "Phoenix Castle owes you a great deal. Ape Mountain may need you now.",
    },
    Giver {
        npc_id: 600_068,
        option: 1,
        name: "OldGeneral",
        greeting: "The mountain is wild, and my soldiers are few. Will you help?",
        all_done: "the mountain is quiet thanks to you. The desert is harder still.",
    },
    Giver {
        npc_id: 30_104,
        option: 1,
        name: "GeneralShou",
        greeting: "Only the strong cross the desert. Are you strong enough to keep it safe?",
        all_done: "the caravans travel safely again. Look for work on Bird Island.",
    },
    Giver {
        npc_id: 600_062,
        option: 1,
        name: "GossiperWang",
        greeting: "Have you heard? Strange things happen on this island. Care to find out?",
        all_done: "I have nothing left to gossip about, thanks to you!",
    },
    Giver {
        npc_id: 30_102,
        option: 1,
        name: "VillageHead",
        greeting: "The Mystic Castle wakes every night, and our village is the first it reaches. Only a seasoned hero can help.",
        all_done: "the castle sleeps again. Mike, in the market, has harder work.",
    },
    Giver {
        npc_id: 30_164,
        option: 1,
        name: "Mike",
        greeting: "I deal in rare things from the deepest places. Strong enough to fetch them?",
        all_done: "there is nothing left down there I could ask of you. You are a legend.",
    },
];

pub fn giver(npc_id: u32) -> Option<&'static Giver> {
    GIVERS.iter().find(|giver| giver.npc_id == npc_id)
}

/// Whether `option` of `npc_id` asks for a task.
pub fn asks_for_task(npc_id: u32, option: u8) -> bool {
    giver(npc_id).is_some_and(|giver| giver.option == option)
}

/// The opening dialog of the givers other than Yang (who has his own).
pub fn opening(npc_id: u32, _character: &Character) -> Option<(String, Vec<(u8, String)>, u16)> {
    let giver = giver(npc_id).filter(|giver| !giver.greeting.is_empty())?;
    Some((
        format!("{}: {}", giver.name, giver.greeting),
        vec![(giver.option, "Do you have a task for me?".to_owned()), (255, "Not now.".to_owned())],
        47,
    ))
}

/// What a giver says, and does, when asked for a task.
pub async fn ask(state: &State, actor: &Actor<ActorState>, character: &Character, npc_id: u32) -> Result<(), Error> {
    let Some(giver) = giver(npc_id) else {
        return Ok(());
    };
    let pool = state.pool();
    let level = character.entity().level();
    let line = match crate::quests::offer(pool, character.database_id(), level, npc_id).await? {
        Offer::Started(quest) => format!("Quest accepted: {}. {}", quest.name, quest.description),
        Offer::Active(quest, progress) if quest.target_item.is_some() => {
            let before = character.inventory_items();
            match crate::quests::turn_in(pool, character, quest.clone()).await? {
                TurnIn::Completed(done) => {
                    // The items left the bag; the reward came in.
                    let after = character.inventory_items();
                    for item in before.iter().filter(|item| after.iter().all(|kept| kept.uid != item.uid)) {
                        actor.send(MsgItemInfo::remove_inventory_item(*item)).await?;
                    }
                    for item in after.iter().filter(|item| before.iter().all(|had| had.uid != item.uid)) {
                        actor.send(MsgItemInfo::inventory_item(*item)).await?;
                    }
                    character.save(state).await?;
                    actor.send(MsgUserInfo::from(character)).await?;
                    if let Some(level) = done.leveled_to {
                        crate::leveling::after_level_up(state, character, level).await?;
                    }
                    let reward = done
                        .definition
                        .reward_item
                        .map(|item| format!(" and {}", crate::quests::item_name(item)))
                        .unwrap_or_default();
                    format!(
                        "Quest complete: {}. Reward: {} silver, {} experience{reward}.",
                        done.definition.name, done.reward_silver, done.reward_experience
                    )
                },
                TurnIn::Missing(have) => format!(
                    "{}: {} You have {have} of {}.",
                    giver.name, quest.description, quest.required_count
                ),
                TurnIn::BagFull => "Make room in your bag for the reward first.".to_owned(),
                TurnIn::NotDelivery => format!("Finish what you started first: {} ({progress}).", quest.name),
            }
        },
        Offer::Active(quest, _) if quest.target_npc.is_some() => {
            format!("{}: {}", giver.name, quest.description)
        },
        Offer::Active(quest, progress) => format!(
            "Finish what you started first: {} ({}/{}).",
            quest.name, progress, quest.required_count
        ),
        Offer::TooLow(quest) => format!("{}: come back at level {} for {}.", giver.name, quest.min_level, quest.name),
        Offer::AllDone => format!("{}: {}", giver.name, giver.all_done),
        Offer::Nothing => "This quest is not available.".to_owned(),
    };
    actor
        .send(MsgTalk::from_system(character.id(), TalkChannel::System, line))
        .await?;
    crate::packets::send_quest_log(state, character).await
}

/// Speaking to `npc_id` delivers the messages meant for it.
pub async fn arrive(state: &State, actor: &Actor<ActorState>, character: &Character, npc_id: u32) -> Result<(), Error> {
    let done = crate::quests::talked_to(state.pool(), character, npc_id).await?;
    if done.is_empty() {
        return Ok(());
    }
    character.save(state).await?;
    actor.send(MsgUserInfo::from(character)).await?;
    for quest in done {
        if let Some(item) = quest.reward_item {
            actor.send(MsgItemInfo::inventory_item(item)).await?;
        }
        if let Some(level) = quest.leveled_to {
            crate::leveling::after_level_up(state, character, level).await?;
        }
        actor
            .send(MsgTalk::from_system(
                character.id(),
                TalkChannel::System,
                format!(
                    "Quest complete: {}. Reward: {} silver, {} experience.",
                    quest.definition.name, quest.reward_silver, quest.reward_experience
                ),
            ))
            .await?;
    }
    crate::packets::send_quest_log(state, character).await
}

/// A kill for the collections under way: the items it gave, in the bag.
pub async fn collect(state: &State, character: &Character, monster_type_id: u32) -> Result<(), Error> {
    let given = crate::quests::collect(
        state.pool(),
        state.shop_catalog(),
        character,
        monster_type_id,
        crate::combat::next_roll,
    )
    .await?;
    let owner = character.owner();
    for (quest, item) in given {
        owner.send(MsgItemInfo::inventory_item(item)).await?;
        crate::packets::send_quest_progress(character, &quest, 0).await?;
        let have = quest.target_item.map_or(0, |item_type| character.count_items(item_type));
        owner
            .send(MsgTalk::from_system(
                character.id(),
                TalkChannel::TopLeft,
                format!("{}: {have}/{} collected.", quest.name, quest.required_count),
            ))
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_giver_has_its_task_option() {
        assert!(super::asks_for_task(425, 4) && !super::asks_for_task(425, 1));
        assert!(super::asks_for_task(30_081, 1));
        assert!(!super::asks_for_task(10_050, 1), "a Conductress is no giver");
        assert_eq!(super::GIVERS.iter().filter(|giver| giver.greeting.is_empty()).count(), 1, "only Yang has his own dialog");
    }
}
