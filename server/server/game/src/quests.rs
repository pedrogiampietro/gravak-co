//! Quests: chains of monster hunts offered by NPCs. An NPC offers the next
//! quest of its chain a character may take, once the quest before it is done
//! and the character has the level; a kill counts toward every active quest
//! for that monster, and the last one pays silver, experience and maybe an
//! item.

use crate::entities::{Character, InventoryItem};
use crate::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestDefinition {
    pub quest_id: u32,
    pub npc_id: u32,
    pub name: String,
    pub description: String,
    pub target_monster_type: u32,
    pub required_count: u32,
    pub reward_silver: u32,
    pub reward_experience: u64,
    /// The quest that must be completed first.
    pub required_quest: Option<u32>,
    pub min_level: u16,
    /// An item type given on completion.
    pub reward_item: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartOutcome {
    Started(QuestDefinition),
    AlreadyActive(QuestDefinition, u32),
    AlreadyCompleted(QuestDefinition),
    NotFound,
}

/// What an NPC has for a character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Offer {
    /// The next quest of the chain, now started.
    Started(QuestDefinition),
    /// A quest of this NPC already under way, with its progress.
    Active(QuestDefinition, u32),
    /// The next quest needs a higher level.
    TooLow(QuestDefinition),
    /// Every quest of this NPC is done.
    AllDone,
    /// This NPC has no quests.
    Nothing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    pub definition: QuestDefinition,
    pub progress: u32,
    pub reward_silver: u32,
    pub reward_experience: u64,
    /// The reward item, in the bag.
    pub reward_item: Option<InventoryItem>,
    pub leveled_to: Option<u8>,
}

/// What a kill did to a character's quests.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DefeatOutcome {
    pub completed: Vec<Completion>,
    /// Quests one kill from done whose reward item does not fit in the bag:
    /// they wait, so the item is not lost.
    pub waiting_for_room: Vec<QuestDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestState {
    pub definition: QuestDefinition,
    pub progress: u32,
    pub completed: bool,
}

/// The columns of a definition, as [`definition`] reads them.
const COLUMNS: &str = "q.quest_id, q.npc_id, q.name, q.description, q.target_monster_type, q.required_count,
     q.reward_silver, q.reward_experience, q.required_quest, q.min_level, q.reward_item";

type DefinitionRow = (i64, i64, String, String, i64, i64, i64, i64, Option<i64>, i64, Option<i64>);

fn definition(row: DefinitionRow) -> QuestDefinition {
    let (
        quest_id,
        npc_id,
        name,
        description,
        target_monster_type,
        required_count,
        reward_silver,
        reward_experience,
        required_quest,
        min_level,
        reward_item,
    ) = row;
    QuestDefinition {
        quest_id: u32::try_from(quest_id).unwrap_or_default(),
        npc_id: u32::try_from(npc_id).unwrap_or_default(),
        name,
        description,
        target_monster_type: u32::try_from(target_monster_type).unwrap_or_default(),
        required_count: u32::try_from(required_count).unwrap_or_default(),
        reward_silver: u32::try_from(reward_silver).unwrap_or_default(),
        reward_experience: u64::try_from(reward_experience).unwrap_or_default(),
        required_quest: required_quest.and_then(|id| u32::try_from(id).ok()),
        min_level: u16::try_from(min_level).unwrap_or(u16::MAX),
        reward_item: reward_item.and_then(|id| u32::try_from(id).ok()),
    }
}

/// A short name for a reward item in chat.
pub fn item_name(item_type: u32) -> &'static str {
    match item_type {
        1_088_000 => "a DragonBall",
        1_088_001 => "a Meteor",
        _ => "an item",
    }
}

pub async fn start(pool: &sqlx::PgPool, character_id: i32, quest_id: u32) -> Result<StartOutcome, Error> {
    let Some(definition) = load_definition(pool, quest_id).await? else {
        return Ok(StartOutcome::NotFound);
    };
    let existing = sqlx::query_as::<_, (i64, i64)>(
        "SELECT state, progress FROM character_quests WHERE character_id = $1 AND quest_id = $2",
    )
    .bind(i64::from(character_id))
    .bind(i64::from(quest_id))
    .fetch_optional(pool)
    .await?;
    match existing {
        Some((2, _)) => Ok(StartOutcome::AlreadyCompleted(definition)),
        Some((1, progress)) => Ok(StartOutcome::AlreadyActive(
            definition,
            u32::try_from(progress).unwrap_or_default(),
        )),
        Some(_) => Ok(StartOutcome::AlreadyActive(definition, 0)),
        None => {
            // Two clicks at once start it once.
            sqlx::query(
                "INSERT INTO character_quests (character_id, quest_id, progress, state) VALUES ($1, $2, 0, 1)
                 ON CONFLICT DO NOTHING",
            )
            .bind(i64::from(character_id))
            .bind(i64::from(quest_id))
            .execute(pool)
            .await?;
            Ok(StartOutcome::Started(definition))
        },
    }
}

/// What `npc_id` has for a character of `level`: the quest under way, else
/// the next one of its chain (started now), in quest order.
pub async fn offer(pool: &sqlx::PgPool, character_id: i32, level: u16, npc_id: u32) -> Result<Offer, Error> {
    let rows = sqlx::query_as::<_, (i64, i64, String, String, i64, i64, i64, i64, Option<i64>, i64, Option<i64>, Option<i64>, Option<i64>, bool)>(
        &format!(
            "SELECT {COLUMNS}, cq.state, cq.progress,
                    q.required_quest IS NULL OR EXISTS (
                        SELECT 1 FROM character_quests done
                        WHERE done.character_id = $1 AND done.quest_id = q.required_quest AND done.state = 2
                    )
             FROM quest_definitions q
             LEFT JOIN character_quests cq ON cq.quest_id = q.quest_id AND cq.character_id = $1
             WHERE q.npc_id = $2
             ORDER BY q.quest_id"
        ),
    )
    .bind(i64::from(character_id))
    .bind(i64::from(npc_id))
    .fetch_all(pool)
    .await?;
    if rows.is_empty() {
        return Ok(Offer::Nothing);
    }
    let mut next = None;
    for row in rows {
        let (a, b, c, d, e, f, g, h, i, j, k, state, progress, unlocked) = row;
        let quest = definition((a, b, c, d, e, f, g, h, i, j, k));
        match state {
            Some(1) => {
                return Ok(Offer::Active(quest, progress.and_then(|p| u32::try_from(p).ok()).unwrap_or_default()));
            },
            Some(_) => {},
            None if unlocked && next.is_none() => next = Some(quest),
            None => {},
        }
    }
    let Some(quest) = next else {
        return Ok(Offer::AllDone);
    };
    if level < quest.min_level {
        return Ok(Offer::TooLow(quest));
    }
    match start(pool, character_id, quest.quest_id).await? {
        StartOutcome::Started(quest) => Ok(Offer::Started(quest)),
        StartOutcome::AlreadyActive(quest, progress) => Ok(Offer::Active(quest, progress)),
        StartOutcome::AlreadyCompleted(_) | StartOutcome::NotFound => Ok(Offer::AllDone),
    }
}

pub async fn list(pool: &sqlx::PgPool, character_id: i32) -> Result<Vec<QuestState>, Error> {
    let rows = sqlx::query_as::<_, (i64, i64, String, String, i64, i64, i64, i64, Option<i64>, i64, Option<i64>, i64, i64)>(
        &format!(
            "SELECT {COLUMNS}, cq.progress, cq.state
             FROM character_quests cq
             INNER JOIN quest_definitions q ON q.quest_id = cq.quest_id
             WHERE cq.character_id = $1
             ORDER BY cq.state, q.quest_id"
        ),
    )
    .bind(i64::from(character_id))
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(a, b, c, d, e, f, g, h, i, j, k, progress, state)| QuestState {
            definition: definition((a, b, c, d, e, f, g, h, i, j, k)),
            progress: u32::try_from(progress).unwrap_or_default(),
            completed: state == 2,
        })
        .collect())
}

pub async fn record_monster_defeat(
    pool: &sqlx::PgPool,
    character: &Character,
    monster_type_id: u32,
) -> Result<DefeatOutcome, Error> {
    let rows = sqlx::query_as::<_, (i64, i64, String, String, i64, i64, i64, i64, Option<i64>, i64, Option<i64>, i64)>(
        &format!(
            "SELECT {COLUMNS}, cq.progress
             FROM quest_definitions q
             INNER JOIN character_quests cq ON cq.quest_id = q.quest_id
             WHERE cq.character_id = $1 AND cq.state = 1 AND q.target_monster_type = $2
             ORDER BY q.quest_id"
        ),
    )
    .bind(i64::from(character.database_id()))
    .bind(i64::from(monster_type_id))
    .fetch_all(pool)
    .await?;
    let mut outcome = DefeatOutcome::default();
    for (a, b, c, d, e, f, g, h, i, j, k, progress) in rows {
        let quest = definition((a, b, c, d, e, f, g, h, i, j, k));
        let required_count = i64::from(quest.required_count);
        let next_progress = progress.saturating_add(1).min(required_count);
        let finishing = next_progress >= required_count;
        if finishing && quest.reward_item.is_some() && character.inventory_items().len() >= 40 {
            outcome.waiting_for_room.push(quest);
            continue;
        }
        let updated = sqlx::query(
            "UPDATE character_quests
             SET progress = $1, state = CASE WHEN $2 THEN 2 ELSE 1 END,
                 completed_at = CASE WHEN $2 THEN EXTRACT(EPOCH FROM now())::BIGINT ELSE completed_at END
             WHERE character_id = $3 AND quest_id = $4 AND state = 1 AND progress = $5",
        )
        .bind(next_progress)
        .bind(finishing)
        .bind(i64::from(character.database_id()))
        .bind(i64::from(quest.quest_id))
        .bind(progress)
        .execute(pool)
        .await?;
        if updated.rows_affected() != 1 || !finishing {
            continue;
        }
        character.grant_silver(pool, quest.reward_silver).await?;
        crate::audit::economic("quest_reward", character.id(), quest.npc_id, 0, quest.reward_silver);
        let reward_item = match quest.reward_item {
            Some(item_type) => {
                let item = character.add_ground_item(pool, item_type, 100, 100).await?;
                if let Some(item) = &item {
                    crate::audit::item(
                        "quest_reward",
                        character.id(),
                        item.uid,
                        item.item_type,
                        format_args!("quest_id={}", quest.quest_id),
                    );
                }
                item
            },
            None => None,
        };
        let leveled_to = character.grant_experience(quest.reward_experience);
        tracing::info!(
            target: "world",
            character = %character.entity().name(),
            quest = %quest.name,
            "Quest completed"
        );
        outcome.completed.push(Completion {
            progress: quest.required_count,
            reward_silver: quest.reward_silver,
            reward_experience: quest.reward_experience,
            definition: quest,
            reward_item,
            leveled_to,
        });
    }
    Ok(outcome)
}

async fn load_definition(pool: &sqlx::PgPool, quest_id: u32) -> Result<Option<QuestDefinition>, Error> {
    let row = sqlx::query_as::<_, DefinitionRow>(&format!(
        "SELECT {COLUMNS} FROM quest_definitions q WHERE q.quest_id = $1"
    ))
    .bind(i64::from(quest_id))
    .fetch_optional(pool)
    .await?;
    Ok(row.map(definition))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tq_network::Actor;

    async fn test_character() -> Result<(sqlx::PgPool, Character), Error> {
        let pool = tq_db::testing::pool().await;
        let (tx, _rx) = tokio::sync::mpsc::channel(8);
        let actor = Actor::<crate::ActorState>::new(tx);
        actor.set_id(1);
        let inner = crate::packets::MsgRegister::build_character_with(
            "quest-test".to_owned(),
            crate::packets::BodyType::MuscularMale,
            crate::packets::BaseClass::Trojan,
            1,
            1,
        )?;
        inner.save(&pool).await?;
        let inner = tq_db::character::Character::from_account(&pool, 1)
            .await?
            .expect("test character should exist");
        actor.set_id(usize::try_from(inner.character_id).expect("character id should fit actor"));
        Ok((pool, Character::new(actor.handle(), inner)))
    }

    #[tokio::test]
    async fn old_general_yang_offers_his_quests_one_after_another() -> Result<(), Error> {
        let (pool, character) = test_character().await?;
        let id = character.database_id();
        let Offer::Started(first) = offer(&pool, id, 1, 425).await? else {
            panic!("the first quest is offered");
        };
        assert_eq!(first.quest_id, 1);
        assert!(matches!(offer(&pool, id, 1, 425).await?, Offer::Active(quest, 0) if quest.quest_id == 1));
        assert_eq!(record_monster_defeat(&pool, &character, 1).await?.completed.len(), 1);

        // The second needs level 5.
        assert!(matches!(offer(&pool, id, 4, 425).await?, Offer::TooLow(quest) if quest.quest_id == 2));
        assert!(matches!(offer(&pool, id, 5, 425).await?, Offer::Started(quest) if quest.quest_id == 2));
        for _ in 0..9 {
            assert!(record_monster_defeat(&pool, &character, 2).await?.completed.is_empty());
        }
        let done = record_monster_defeat(&pool, &character, 2).await?.completed;
        assert_eq!(done.len(), 1);
        assert_eq!(done[0].reward_item, None);

        // The third pays a Meteor, which waits for room in the bag.
        assert!(matches!(offer(&pool, id, 10, 425).await?, Offer::Started(quest) if quest.quest_id == 3));
        for position in 0..40_u8 {
            sqlx::query(
                "INSERT INTO inventory_items (character_id, item_type, position, durability, max_durability)
                 VALUES ($1, 1000000, $2, 1, 1)",
            )
            .bind(i64::from(id))
            .bind(i64::from(position))
            .execute(&pool)
            .await?;
        }
        character.load_inventory(&pool).await?;
        for _ in 0..14 {
            record_monster_defeat(&pool, &character, 3).await?;
        }
        let full = record_monster_defeat(&pool, &character, 3).await?;
        assert!(full.completed.is_empty());
        assert_eq!(full.waiting_for_room.len(), 1, "the reward waits for a free slot");
        sqlx::query("DELETE FROM inventory_items WHERE character_id = $1 AND position = 7")
            .bind(i64::from(id))
            .execute(&pool)
            .await?;
        character.load_inventory(&pool).await?;
        let done = record_monster_defeat(&pool, &character, 3).await?.completed;
        assert_eq!(done.len(), 1);
        let meteor = done[0].reward_item.expect("the Meteor is given");
        assert_eq!((meteor.item_type, meteor.position), (1_088_001, 7));
        Ok(())
    }

    #[tokio::test]
    async fn quest_progress_and_reward_are_persistent_and_idempotent() -> Result<(), Error> {
        let (pool, character) = test_character().await?;
        let StartOutcome::Started(quest) = start(&pool, character.database_id(), 1).await? else {
            panic!("quest should start once");
        };
        assert_eq!(quest.target_monster_type, 1);
        assert!(matches!(
            start(&pool, character.database_id(), 1).await?,
            StartOutcome::AlreadyActive(_, 0)
        ));
        assert_eq!(list(&pool, character.database_id()).await?.len(), 1);
        assert!(!list(&pool, character.database_id()).await?[0].completed);

        let silver_before = character.silver();
        let completed = record_monster_defeat(&pool, &character, 1).await?.completed;
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].progress, 1);
        assert_eq!(character.silver(), silver_before + 100);
        assert!(record_monster_defeat(&pool, &character, 1).await?.completed.is_empty());
        assert!(matches!(
            start(&pool, character.database_id(), 1).await?,
            StartOutcome::AlreadyCompleted(_)
        ));
        let states = list(&pool, character.database_id()).await?;
        assert_eq!(states[0].progress, 1);
        assert!(states[0].completed);

        let row: (i64, i64) =
            sqlx::query_as("SELECT state, progress FROM character_quests WHERE character_id = $1 AND quest_id = 1")
                .bind(i64::from(character.database_id()))
                .fetch_one(&pool)
                .await?;
        assert_eq!(row, (2, 1));
        Ok(())
    }
}
