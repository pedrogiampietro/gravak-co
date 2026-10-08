//! Gravak's quest log (9102): one packet per quest, as a string holds 255
//! bytes at most. The log is sent whole (a clear, then each quest) when the
//! QUESTS panel opens and after a quest starts or ends; a hunt's kill only
//! updates its own line.

use serde::{Deserialize, Serialize};
use tq_network::PacketID;

use crate::entities::Character;
use crate::quests::{QuestDefinition, QuestKind};
use crate::{Error, State};

/// The log starts over: the quests that follow replace it.
pub const CLEAR: u32 = 0;
/// One quest, new or changed (by `quest_id`).
pub const ENTRY: u32 = 1;

/// Not taken yet (the giver's next), under way, or done.
pub const AVAILABLE: u32 = 0;
pub const ACTIVE: u32 = 1;
pub const DONE: u32 = 2;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, PacketID)]
#[packet(id = 9102)]
pub struct MsgQuestLog {
    pub action: u32,
    pub quest_id: u32,
    pub state: u32,
    /// 0 hunt, 1 delivery, 2 collection, 3 message.
    pub kind: u32,
    pub progress: u32,
    pub required: u32,
    pub min_level: u32,
    pub name: String,
    pub giver: String,
    pub description: String,
}

/// `text` cut to what a packet string holds, on a character boundary.
fn fit(text: &str) -> String {
    let mut end = text.len().min(255);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

fn kind_code(kind: QuestKind) -> u32 {
    match kind {
        QuestKind::Hunt => 0,
        QuestKind::Delivery => 1,
        QuestKind::Collection => 2,
        QuestKind::Message => 3,
    }
}

impl MsgQuestLog {
    pub fn clear() -> Self {
        Self {
            action: CLEAR,
            ..Self::default()
        }
    }

    pub fn entry(quest: &QuestDefinition, state: u32, progress: u32) -> Self {
        let giver = crate::quest_givers::giver(quest.npc_id).map_or("", |giver| giver.name);
        Self {
            action: ENTRY,
            quest_id: quest.quest_id,
            state,
            kind: kind_code(quest.kind()),
            progress,
            required: quest.required_count,
            min_level: u32::from(quest.min_level),
            name: fit(&quest.name),
            giver: fit(giver),
            description: fit(&quest.description),
        }
    }
}

/// What a quest's line shows as done: kills for a hunt, the items in the bag
/// for a delivery or a collection.
fn shown_progress(character: &Character, quest: &QuestDefinition, progress: u32, done: bool) -> u32 {
    if done {
        return quest.required_count;
    }
    match quest.target_item {
        Some(item_type) => u32::try_from(character.count_items(item_type)).unwrap_or(u32::MAX).min(quest.required_count),
        None => progress,
    }
}

/// Sends a character its whole log: the quests under way, then each giver's
/// next, then the ones done.
pub async fn send_log(state: &State, character: &Character) -> Result<(), Error> {
    let owner = character.owner();
    let quests = crate::quests::list(state.pool(), character.database_id()).await?;
    let next = crate::quests::next_offers(state.pool(), character.database_id()).await?;
    owner.send(MsgQuestLog::clear()).await?;
    for quest in quests.iter().filter(|quest| !quest.completed) {
        let progress = shown_progress(character, &quest.definition, quest.progress, false);
        owner.send(MsgQuestLog::entry(&quest.definition, ACTIVE, progress)).await?;
    }
    for quest in &next {
        owner.send(MsgQuestLog::entry(quest, AVAILABLE, 0)).await?;
    }
    for quest in quests.iter().filter(|quest| quest.completed) {
        owner.send(MsgQuestLog::entry(&quest.definition, DONE, quest.required())).await?;
    }
    Ok(())
}

/// Updates one quest's line after a kill.
pub async fn send_progress(character: &Character, quest: &QuestDefinition, progress: u32) -> Result<(), Error> {
    let progress = shown_progress(character, quest, progress, false);
    character.owner().send(MsgQuestLog::entry(quest, ACTIVE, progress)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_texts_fit_a_packet_string() {
        let long = "é".repeat(200);
        let cut = fit(&long);
        assert!(cut.len() <= 255 && long.starts_with(&cut));
        assert_eq!(fit("short"), "short");
    }
}
