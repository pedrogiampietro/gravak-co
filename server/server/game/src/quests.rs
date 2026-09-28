use crate::entities::Character;
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartOutcome {
    Started(QuestDefinition),
    AlreadyActive(QuestDefinition, u32),
    AlreadyCompleted(QuestDefinition),
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    pub definition: QuestDefinition,
    pub progress: u32,
    pub reward_silver: u32,
    pub reward_experience: u64,
    pub leveled_to: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestState {
    pub definition: QuestDefinition,
    pub progress: u32,
    pub completed: bool,
}

pub async fn start(pool: &sqlx::SqlitePool, character_id: i32, quest_id: u32) -> Result<StartOutcome, Error> {
    let Some(definition) = load_definition(pool, quest_id).await? else {
        return Ok(StartOutcome::NotFound);
    };
    let existing = sqlx::query_as::<_, (i64, i64)>(
        "SELECT state, progress FROM character_quests WHERE character_id = ? AND quest_id = ?",
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
            sqlx::query("INSERT INTO character_quests (character_id, quest_id, progress, state) VALUES (?, ?, 0, 1)")
                .bind(i64::from(character_id))
                .bind(i64::from(quest_id))
                .execute(pool)
                .await?;
            Ok(StartOutcome::Started(definition))
        },
    }
}

pub async fn list(pool: &sqlx::SqlitePool, character_id: i32) -> Result<Vec<QuestState>, Error> {
    let rows = sqlx::query_as::<_, (i64, i64, String, String, i64, i64, i64, i64, i64, i64)>(
        "SELECT q.quest_id, q.npc_id, q.name, q.description, q.target_monster_type,
                q.required_count, q.reward_silver, q.reward_experience,
                cq.progress, cq.state
         FROM character_quests cq
         INNER JOIN quest_definitions q ON q.quest_id = cq.quest_id
         WHERE cq.character_id = ?
         ORDER BY cq.state, q.quest_id",
    )
    .bind(i64::from(character_id))
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(
                quest_id,
                npc_id,
                name,
                description,
                target_monster_type,
                required_count,
                reward_silver,
                reward_experience,
                progress,
                state,
            )| QuestState {
                definition: QuestDefinition {
                    quest_id: u32::try_from(quest_id).unwrap_or_default(),
                    npc_id: u32::try_from(npc_id).unwrap_or_default(),
                    name,
                    description,
                    target_monster_type: u32::try_from(target_monster_type).unwrap_or_default(),
                    required_count: u32::try_from(required_count).unwrap_or_default(),
                    reward_silver: u32::try_from(reward_silver).unwrap_or_default(),
                    reward_experience: u64::try_from(reward_experience).unwrap_or_default(),
                },
                progress: u32::try_from(progress).unwrap_or_default(),
                completed: state == 2,
            },
        )
        .collect())
}

pub async fn record_monster_defeat(
    pool: &sqlx::SqlitePool,
    character: &Character,
    monster_type_id: u32,
) -> Result<Vec<Completion>, Error> {
    let rows = sqlx::query_as::<_, (i64, i64, String, String, i64, i64, i64, i64, i64)>(
        "SELECT q.quest_id, q.npc_id, q.name, q.description, q.target_monster_type,
                q.required_count, q.reward_silver, q.reward_experience, cq.progress
         FROM quest_definitions q
         INNER JOIN character_quests cq ON cq.quest_id = q.quest_id
         WHERE cq.character_id = ? AND cq.state = 1 AND q.target_monster_type = ?
         ORDER BY q.quest_id",
    )
    .bind(i64::from(character.database_id()))
    .bind(i64::from(monster_type_id))
    .fetch_all(pool)
    .await?;
    let mut completed = Vec::new();
    for (
        quest_id,
        npc_id,
        name,
        description,
        target_monster_type,
        required_count,
        reward_silver,
        reward_experience,
        progress,
    ) in rows
    {
        let next_progress = progress.saturating_add(1).min(required_count);
        let updated = sqlx::query(
            "UPDATE character_quests
             SET progress = ?, state = CASE WHEN ? >= ? THEN 2 ELSE 1 END,
                 completed_at = CASE WHEN ? >= ? THEN unixepoch() ELSE completed_at END
             WHERE character_id = ? AND quest_id = ? AND state = 1 AND progress = ?",
        )
        .bind(next_progress)
        .bind(next_progress)
        .bind(required_count)
        .bind(next_progress)
        .bind(required_count)
        .bind(i64::from(character.database_id()))
        .bind(quest_id)
        .bind(progress)
        .execute(pool)
        .await?;
        if updated.rows_affected() != 1 || next_progress < required_count {
            continue;
        }
        let reward_silver = u32::try_from(reward_silver).unwrap_or_default();
        let reward_experience = u64::try_from(reward_experience).unwrap_or_default();
        character.grant_silver(pool, reward_silver).await?;
        crate::audit::economic(
            "quest_reward",
            character.id(),
            u32::try_from(npc_id).unwrap_or_default(),
            0,
            reward_silver,
        );
        let leveled_to = character.grant_experience(reward_experience);
        completed.push(Completion {
            definition: QuestDefinition {
                quest_id: u32::try_from(quest_id).unwrap_or_default(),
                npc_id: u32::try_from(npc_id).unwrap_or_default(),
                name,
                description,
                target_monster_type: u32::try_from(target_monster_type).unwrap_or_default(),
                required_count: u32::try_from(required_count).unwrap_or_default(),
                reward_silver,
                reward_experience,
            },
            progress: u32::try_from(next_progress).unwrap_or_default(),
            reward_silver,
            reward_experience,
            leveled_to,
        });
    }
    Ok(completed)
}

async fn load_definition(pool: &sqlx::SqlitePool, quest_id: u32) -> Result<Option<QuestDefinition>, Error> {
    let row = sqlx::query_as::<_, (i64, i64, String, String, i64, i64, i64, i64)>(
        "SELECT quest_id, npc_id, name, description, target_monster_type, required_count,
                reward_silver, reward_experience
         FROM quest_definitions WHERE quest_id = ?",
    )
    .bind(i64::from(quest_id))
    .fetch_optional(pool)
    .await?;
    Ok(row.map(
        |(
            quest_id,
            npc_id,
            name,
            description,
            target_monster_type,
            required_count,
            reward_silver,
            reward_experience,
        )| {
            QuestDefinition {
                quest_id: u32::try_from(quest_id).unwrap_or_default(),
                npc_id: u32::try_from(npc_id).unwrap_or_default(),
                name,
                description,
                target_monster_type: u32::try_from(target_monster_type).unwrap_or_default(),
                required_count: u32::try_from(required_count).unwrap_or_default(),
                reward_silver: u32::try_from(reward_silver).unwrap_or_default(),
                reward_experience: u64::try_from(reward_experience).unwrap_or_default(),
            }
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;
    use tq_network::Actor;

    async fn test_character() -> Result<(sqlx::SqlitePool, Character), Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(2)
            .connect("sqlite::memory:")
            .await?;
        sqlx::migrate!("../../migrations")
            .run(&pool)
            .await
            .map_err(|error| Error::Other(error.to_string()))?;
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
        let completed = record_monster_defeat(&pool, &character, 1).await?;
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].progress, 1);
        assert_eq!(character.silver(), silver_before + 100);
        assert!(record_monster_defeat(&pool, &character, 1).await?.is_empty());
        assert!(matches!(
            start(&pool, character.database_id(), 1).await?,
            StartOutcome::AlreadyCompleted(_)
        ));
        let states = list(&pool, character.database_id()).await?;
        assert_eq!(states[0].progress, 1);
        assert!(states[0].completed);

        let row: (i64, i64) =
            sqlx::query_as("SELECT state, progress FROM character_quests WHERE character_id = ? AND quest_id = 1")
                .bind(i64::from(character.database_id()))
                .fetch_one(&pool)
                .await?;
        assert_eq!(row, (2, 1));
        Ok(())
    }
}
