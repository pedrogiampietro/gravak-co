//! Bounty tasks, after Tibia's: a character picks one of three monsters of a
//! difficulty, defeats enough of them, and earns experience, bounty points and
//! a reroll token. Reroll tokens (ten at most, one free a day) bring a new list
//! or a new difficulty, abandoning the task under way. Bounty points upgrade
//! the talisman, whose four bonuses work against the monster of the task:
//! more damage, life leech, an extra loot roll and kills that count twice.
//!
//! Conquer has no bestiary, so a difficulty is a band of monster levels, and
//! no extra equipment slot, so the talisman belongs to the character.

use std::collections::HashMap;

use parking_lot::Mutex;

use crate::Error;

/// Reroll tokens a character can hold.
pub const MAX_TOKENS: u32 = 10;
/// Talisman levels per bonus.
pub const MAX_TALISMAN_LEVEL: u32 = 40;
/// Monsters with fewer spawned than this (bosses, rare ones) are never asked.
const MIN_SPAWNED: i64 = 10;
/// Names each of a character's lists holds.
pub const MAX_LISTED: usize = 3;
/// Bounty points a name costs on the preferred list, and on the avoided one.
pub const PREFER_COST: u32 = 10;
pub const AVOID_COST: u32 = 5;
/// How many times likelier a preferred monster is to be offered.
const PREFERRED_WEIGHT: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Difficulty {
    Beginner = 0,
    Adept = 1,
    Expert = 2,
    Master = 3,
}

impl Difficulty {
    pub fn from_u32(value: u32) -> Option<Self> {
        [Self::Beginner, Self::Adept, Self::Expert, Self::Master]
            .into_iter()
            .find(|difficulty| *difficulty as u32 == value)
    }

    /// The monster levels it draws from.
    pub fn levels(self) -> (u16, u16) {
        match self {
            Self::Beginner => (1, 35),
            Self::Adept => (20, 70),
            Self::Expert => (55, 105),
            Self::Master => (90, 140),
        }
    }

    /// The kills a Normal task asks, at least and at most.
    pub fn kills(self) -> (u32, u32) {
        match self {
            Self::Beginner => (50, 110),
            Self::Adept => (80, 190),
            Self::Expert => (150, 310),
            Self::Master => (300, 600),
        }
    }

    /// Bounty points of a Normal task.
    pub fn points(self) -> u32 {
        match self {
            Self::Beginner => 3,
            Self::Adept => 7,
            Self::Expert => 16,
            Self::Master => 27,
        }
    }
}

/// Silver and Gold tasks ask for more kills and pay more.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grade {
    Normal = 0,
    Silver = 1,
    Gold = 2,
}

impl Grade {
    /// 70% Normal, 22% Silver, 8% Gold.
    pub fn roll(roll: u32) -> Self {
        match roll % 100 {
            0..=69 => Self::Normal,
            70..=91 => Self::Silver,
            _ => Self::Gold,
        }
    }

    pub fn from_u32(value: u32) -> Self {
        match value {
            1 => Self::Silver,
            2 => Self::Gold,
            _ => Self::Normal,
        }
    }

    /// Percent applied to kills, points and experience.
    fn percents(self) -> (u32, u32, u32) {
        match self {
            Self::Normal => (100, 100, 100),
            Self::Silver => (150, 150, 175),
            Self::Gold => (200, 250, 300),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bonus {
    Damage = 0,
    Leech = 1,
    Loot = 2,
    DoubleKill = 3,
}

impl Bonus {
    pub fn from_u32(value: u32) -> Option<Self> {
        [Self::Damage, Self::Leech, Self::Loot, Self::DoubleKill]
            .into_iter()
            .find(|bonus| *bonus as u32 == value)
    }

    fn column(self) -> &'static str {
        match self {
            Self::Damage => "damage_level",
            Self::Leech => "leech_level",
            Self::Loot => "loot_level",
            Self::DoubleKill => "double_kill_level",
        }
    }

    /// The bonus at `level`, in hundredths of a percent: 0.5% a level up to
    /// 10%, then 0.25% (double kills: 1% a level up to 20%, then 0.5%).
    pub fn basis_points(self, level: u32) -> u32 {
        let level = level.min(MAX_TALISMAN_LEVEL);
        let (step, after) = match self {
            Self::DoubleKill => (100, 50),
            _ => (50, 25),
        };
        level.min(20) * step + level.saturating_sub(20) * after
    }
}

/// Bounty points the next talisman level costs.
pub fn upgrade_cost(level: u32) -> u32 {
    5 + 12 * level
}

/// A monster a task may ask for. Several monster types can share a name
/// (one per spawn area); a task asks for the name, and any of them counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub monster_type: u32,
    pub name: String,
    pub level: u16,
}

/// The monsters a character prefers, and the ones it avoids, by name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lists {
    pub preferred: Vec<String>,
    pub avoided: Vec<String>,
}

/// One of the three offers, or the task under way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub monster_type: u32,
    pub monster_name: String,
    pub kills: u32,
    pub kills_done: u32,
    pub grade: Grade,
    pub points: u32,
    pub experience: u64,
}

/// Up to three offers of `difficulty` from `candidates`, each roll in `rolls`
/// used once (the caller passes random numbers; tests pass fixed ones).
/// Avoided monsters are left out (unless nothing else fits), and preferred
/// ones come up [`PREFERRED_WEIGHT`] times as often.
pub fn roll_offers(
    candidates: &[Candidate],
    difficulty: Difficulty,
    lists: &Lists,
    rolls: &mut impl Iterator<Item = u32>,
) -> Vec<(Candidate, u32, Grade, u32, u64)> {
    let (low, high) = difficulty.levels();
    let in_band: Vec<&Candidate> = candidates
        .iter()
        .filter(|candidate| (low..=high).contains(&candidate.level))
        .collect();
    let wanted: Vec<&Candidate> = in_band
        .iter()
        .copied()
        .filter(|candidate| !lists.avoided.contains(&candidate.name))
        .collect();
    let mut pool = if wanted.is_empty() { in_band } else { wanted };
    let weight = |candidate: &Candidate| {
        if lists.preferred.contains(&candidate.name) { PREFERRED_WEIGHT } else { 1 }
    };
    let mut offers = Vec::new();
    while offers.len() < 3 && !pool.is_empty() {
        let total: u32 = pool.iter().map(|candidate| weight(candidate)).sum();
        let mut target = rolls.next().unwrap_or(0) % total;
        let index = pool
            .iter()
            .position(|candidate| {
                let weight = weight(candidate);
                if target < weight {
                    return true;
                }
                target -= weight;
                false
            })
            .unwrap_or(0);
        let pick = pool.remove(index);
        let grade = Grade::roll(rolls.next().unwrap_or(0));
        let (kills_percent, points_percent, experience_percent) = grade.percents();
        let (fewest, most) = difficulty.kills();
        let kills = fewest + rolls.next().unwrap_or(0) % (most - fewest + 1);
        let kills = kills * kills_percent / 100;
        let points = (difficulty.points() * points_percent).div_ceil(100);
        // A Normal hunt of a hundred pays half a level at the monster's level.
        let level = u8::try_from(pick.level).unwrap_or(u8::MAX).max(1);
        let experience = crate::leveling::required(level) * u64::from(kills) / 200 * u64::from(experience_percent) / 100;
        offers.push((pick.clone(), kills, grade, points, experience));
    }
    offers
}

/// The talisman's bonuses against the monster of a character's task, kept in
/// memory for combat.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Talisman {
    /// The bounty's monster, by name: any of its types counts.
    pub monster_name: String,
    pub damage_bp: u32,
    pub leech_bp: u32,
    pub loot_bp: u32,
    pub double_kill_bp: u32,
}

impl Talisman {
    /// Whether `monster_name` is the bounty's monster.
    pub fn hunts(&self, monster_name: &str) -> bool {
        !self.monster_name.is_empty() && self.monster_name == monster_name
    }

    /// `damage` with the damage bonus, when the monster is the task's.
    pub fn boost(&self, monster_name: &str, damage: u32) -> u32 {
        if !self.hunts(monster_name) || damage == 0 {
            return damage;
        }
        let boosted = u64::from(damage) * u64::from(10_000 + self.damage_bp) / 10_000;
        u32::try_from(boosted).unwrap_or(u32::MAX)
    }

    /// Life leeched from `damage` dealt to the monster.
    pub fn leech(&self, monster_name: &str, damage: u32) -> u32 {
        if !self.hunts(monster_name) {
            return 0;
        }
        u32::try_from(u64::from(damage) * u64::from(self.leech_bp) / 10_000).unwrap_or(u32::MAX)
    }

    /// Whether a `roll` (0..10000) earns an extra loot roll from the monster.
    pub fn extra_loot(&self, monster_name: &str, roll: u32) -> bool {
        self.hunts(monster_name) && roll % 10_000 < self.loot_bp
    }

    /// Whether a `roll` (0..10000) makes a kill of the monster count twice.
    pub fn double_kill(&self, monster_name: &str, roll: u32) -> bool {
        self.hunts(monster_name) && roll % 10_000 < self.double_kill_bp
    }
}

/// Raises a hit on the task's monster by the talisman's damage bonus, never
/// past the life the monster had.
pub fn empower(talisman: &Talisman, monster_name: &str, outcome: &mut crate::combat::DamageOutcome, life_before: u32) {
    let damage = talisman.boost(monster_name, outcome.damage).min(life_before);
    outcome.damage = damage;
    outcome.remaining_health = life_before - damage;
    outcome.defeated = outcome.remaining_health == 0;
}

/// The talismans of the characters online with a task, by entity id.
#[derive(Debug, Default)]
pub struct TalismanBook {
    talismans: Mutex<HashMap<u32, Talisman>>,
}

impl TalismanBook {
    pub fn get(&self, character_id: u32) -> Option<Talisman> {
        self.talismans.lock().get(&character_id).cloned()
    }

    pub fn set(&self, character_id: u32, talisman: Option<Talisman>) {
        let mut talismans = self.talismans.lock();
        match talisman {
            Some(talisman) => talismans.insert(character_id, talisman),
            None => talismans.remove(&character_id),
        };
    }
}

/// Everything the Task Board shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    pub points: u32,
    pub tokens: u32,
    pub daily_available: bool,
    pub difficulty: Difficulty,
    /// Talisman levels by [`Bonus`].
    pub talisman: [u32; 4],
    pub completed: u32,
    pub offers: Vec<Task>,
    pub task: Option<Task>,
    pub lists: Lists,
}

impl Board {
    /// The talisman against the task under way.
    pub fn talisman(&self) -> Option<Talisman> {
        let task = self.task.as_ref()?;
        Some(Talisman {
            monster_name: task.monster_name.clone(),
            damage_bp: Bonus::Damage.basis_points(self.talisman[0]),
            leech_bp: Bonus::Leech.basis_points(self.talisman[1]),
            loot_bp: Bonus::Loot.basis_points(self.talisman[2]),
            double_kill_bp: Bonus::DoubleKill.basis_points(self.talisman[3]),
        })
    }
}

/// Why a Task Board action was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    NoTokens,
    TaskUnderWay,
    NoSuchOffer,
    NotEnoughPoints(u32),
    MaxLevel,
    DailyTaken,
    TokensFull,
    NoMonsters,
    NoSuchMonster,
    AlreadyListed,
    ListFull,
    NotListed,
}

impl Refusal {
    pub fn message(self) -> String {
        match self {
            Self::NoTokens => "You have no reroll tokens left.".to_owned(),
            Self::TaskUnderWay => "Finish or abandon your bounty first.".to_owned(),
            Self::NoSuchOffer => "That bounty is no longer on offer.".to_owned(),
            Self::NotEnoughPoints(cost) => format!("That upgrade costs {cost} bounty points."),
            Self::MaxLevel => "That talisman bonus is at its highest level.".to_owned(),
            Self::DailyTaken => "You already took today's reroll token.".to_owned(),
            Self::TokensFull => format!("You already hold {MAX_TOKENS} reroll tokens."),
            Self::NoMonsters => "No monsters of that difficulty roam the world yet.".to_owned(),
            Self::NoSuchMonster => "That monster is not known.".to_owned(),
            Self::AlreadyListed => "That monster is already on one of your lists.".to_owned(),
            Self::ListFull => format!("That list already holds {MAX_LISTED} monsters."),
            Self::NotListed => "That monster is no longer on your list.".to_owned(),
        }
    }
}

/// A completed bounty's reward, already given in the database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reward {
    pub monster_name: String,
    pub points: u32,
    pub experience: u64,
    pub tokens: u32,
}

/// What kills did to a character's bounty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    /// Not the task's monster, or no task.
    None,
    Counted { done: u32, required: u32 },
    Completed(Reward),
}

/// Days since 1970 (UTC), for the daily token.
pub fn today() -> i64 {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    i64::try_from(seconds / 86_400).unwrap_or_default()
}

fn random_rolls() -> impl Iterator<Item = u32> {
    std::iter::repeat_with(crate::combat::next_roll)
}

async fn candidates(pool: &sqlx::PgPool) -> Result<Vec<Candidate>, Error> {
    // One per name; not guards, not the few-of-a-kind (bosses), and not the
    // training dummies of the tactics maps (life far beyond their level).
    let rows: Vec<(i64, String, i64)> = sqlx::query_as(
        "SELECT MIN(t.id), t.name, MAX(t.level)
         FROM monster_types t
         JOIN monster_spawns s ON s.monster_type_id = t.id
         WHERE t.name NOT ILIKE '%guard%' AND t.life <= 1000 * GREATEST(t.level, 1)
         GROUP BY t.name
         HAVING SUM(s.amount) >= $1
         ORDER BY MIN(t.id)",
    )
    .bind(MIN_SPAWNED)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, name, level)| Candidate {
            monster_type: u32::try_from(id).unwrap_or_default(),
            name,
            level: u16::try_from(level).unwrap_or_default(),
        })
        .collect())
}

/// A character's preferred and avoided monsters, oldest first.
async fn lists<'e>(executor: impl sqlx::PgExecutor<'e>, character_id: i64) -> Result<Lists, Error> {
    let rows: Vec<(String, bool)> = sqlx::query_as(
        "SELECT monster_name, avoided FROM bounty_lists WHERE character_id = $1 ORDER BY listed_at, monster_name",
    )
    .bind(character_id)
    .fetch_all(executor)
    .await?;
    let mut lists = Lists::default();
    for (name, avoided) in rows {
        if avoided {
            lists.avoided.push(name);
        } else {
            lists.preferred.push(name);
        }
    }
    Ok(lists)
}

/// Puts the monster of `monster_type` on the preferred or the avoided list,
/// for its cost in bounty points; the monster's name.
pub async fn list_monster(
    pool: &sqlx::PgPool,
    character_id: i32,
    monster_type: u32,
    avoided: bool,
) -> Result<Result<String, Refusal>, Error> {
    let board = board(pool, character_id).await?;
    let name: Option<String> = sqlx::query_scalar("SELECT name FROM monster_types WHERE id = $1")
        .bind(i64::from(monster_type))
        .fetch_optional(pool)
        .await?;
    let Some(name) = name else {
        return Ok(Err(Refusal::NoSuchMonster));
    };
    if board.lists.preferred.contains(&name) || board.lists.avoided.contains(&name) {
        return Ok(Err(Refusal::AlreadyListed));
    }
    let list = if avoided { &board.lists.avoided } else { &board.lists.preferred };
    if list.len() >= MAX_LISTED {
        return Ok(Err(Refusal::ListFull));
    }
    let cost = if avoided { AVOID_COST } else { PREFER_COST };
    let id = i64::from(character_id);
    let mut transaction = pool.begin().await?;
    let paid = sqlx::query("UPDATE bounty_profiles SET points = points - $2 WHERE character_id = $1 AND points >= $2")
        .bind(id)
        .bind(i64::from(cost))
        .execute(&mut *transaction)
        .await?;
    if paid.rows_affected() != 1 {
        return Ok(Err(Refusal::NotEnoughPoints(cost)));
    }
    let listed = sqlx::query(
        "INSERT INTO bounty_lists (character_id, monster_name, avoided) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(id)
    .bind(&name)
    .bind(avoided)
    .execute(&mut *transaction)
    .await?;
    if listed.rows_affected() != 1 {
        return Ok(Err(Refusal::AlreadyListed));
    }
    transaction.commit().await?;
    Ok(Ok(name))
}

/// Takes the `index`th name off a list (the points are not given back).
pub async fn unlist(
    pool: &sqlx::PgPool,
    character_id: i32,
    avoided: bool,
    index: usize,
) -> Result<Result<String, Refusal>, Error> {
    let id = i64::from(character_id);
    let lists = lists(pool, id).await?;
    let list = if avoided { lists.avoided } else { lists.preferred };
    let Some(name) = list.into_iter().nth(index) else {
        return Ok(Err(Refusal::NotListed));
    };
    let removed = sqlx::query("DELETE FROM bounty_lists WHERE character_id = $1 AND monster_name = $2")
        .bind(id)
        .bind(&name)
        .execute(pool)
        .await?;
    Ok(if removed.rows_affected() == 1 { Ok(name) } else { Err(Refusal::NotListed) })
}

/// Writes a new list of offers of `difficulty`; false when no monster fits.
async fn write_offers(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    candidates: &[Candidate],
    character_id: i64,
    difficulty: Difficulty,
) -> Result<bool, Error> {
    sqlx::query("DELETE FROM bounty_offers WHERE character_id = $1")
        .bind(character_id)
        .execute(&mut **transaction)
        .await?;
    let lists = lists(&mut **transaction, character_id).await?;
    let offers = roll_offers(candidates, difficulty, &lists, &mut random_rolls());
    for (slot, (candidate, kills, grade, points, experience)) in offers.iter().enumerate() {
        sqlx::query(
            "INSERT INTO bounty_offers (character_id, slot, monster_type, kills, grade, points, experience)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(character_id)
        .bind(slot as i64)
        .bind(i64::from(candidate.monster_type))
        .bind(i64::from(*kills))
        .bind(*grade as i64)
        .bind(i64::from(*points))
        .bind(i64::try_from(*experience).unwrap_or(i64::MAX))
        .execute(&mut **transaction)
        .await?;
    }
    Ok(!offers.is_empty())
}

/// The Task Board of a character, made on first use with a free Beginner list.
pub async fn board(pool: &sqlx::PgPool, character_id: i32) -> Result<Board, Error> {
    let id = i64::from(character_id);
    sqlx::query("INSERT INTO bounty_profiles (character_id) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(id)
        .execute(pool)
        .await?;
    // A new board, or one whose offers were cleared, gets a free list of its
    // difficulty.
    let (empty, difficulty): (bool, i64) = sqlx::query_as(
        "SELECT NOT EXISTS (SELECT 1 FROM bounty_offers WHERE character_id = $1)
                AND NOT EXISTS (SELECT 1 FROM bounty_tasks WHERE character_id = $1),
                difficulty
         FROM bounty_profiles WHERE character_id = $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    if empty {
        let difficulty =
            Difficulty::from_u32(u32::try_from(difficulty).unwrap_or_default()).unwrap_or(Difficulty::Beginner);
        let candidates = candidates(pool).await?;
        let mut transaction = pool.begin().await?;
        write_offers(&mut transaction, &candidates, id, difficulty).await?;
        transaction.commit().await?;
    }
    let (points, tokens, claimed_on, difficulty, damage, leech, loot, double_kill, completed): (
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
    ) = sqlx::query_as(
        "SELECT points, reroll_tokens, daily_claimed_on, difficulty,
                damage_level, leech_level, loot_level, double_kill_level, completed
         FROM bounty_profiles WHERE character_id = $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    let offers: Vec<(i64, String, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT o.monster_type, COALESCE(t.name, ''), o.kills, o.grade, o.points, o.experience
         FROM bounty_offers o LEFT JOIN monster_types t ON t.id = o.monster_type
         WHERE o.character_id = $1 ORDER BY o.slot",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    let task: Option<(i64, String, i64, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT b.monster_type, COALESCE(t.name, ''), b.kills_required, b.kills_done, b.grade, b.points, b.experience
         FROM bounty_tasks b LEFT JOIN monster_types t ON t.id = b.monster_type
         WHERE b.character_id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    let lists = lists(pool, id).await?;
    let number = |value: i64| u32::try_from(value).unwrap_or_default();
    let tokens = number(tokens);
    Ok(Board {
        points: number(points),
        tokens,
        daily_available: claimed_on < today() && tokens < MAX_TOKENS,
        difficulty: Difficulty::from_u32(number(difficulty)).unwrap_or(Difficulty::Beginner),
        talisman: [number(damage), number(leech), number(loot), number(double_kill)],
        completed: number(completed),
        offers: offers
            .into_iter()
            .map(|(monster_type, monster_name, kills, grade, points, experience)| Task {
                monster_type: number(monster_type),
                monster_name,
                kills: number(kills),
                kills_done: 0,
                grade: Grade::from_u32(number(grade)),
                points: number(points),
                experience: u64::try_from(experience).unwrap_or_default(),
            })
            .collect(),
        task: task.map(|(monster_type, monster_name, kills, done, grade, points, experience)| Task {
            monster_type: number(monster_type),
            monster_name,
            kills: number(kills),
            kills_done: number(done),
            grade: Grade::from_u32(number(grade)),
            points: number(points),
            experience: u64::try_from(experience).unwrap_or_default(),
        }),
        lists,
    })
}

/// Spends a token for a new list of `difficulty`, abandoning the task under
/// way.
pub async fn reroll(pool: &sqlx::PgPool, character_id: i32, difficulty: Difficulty) -> Result<Result<(), Refusal>, Error> {
    board(pool, character_id).await?;
    let candidates = candidates(pool).await?;
    let id = i64::from(character_id);
    let mut transaction = pool.begin().await?;
    let spent = sqlx::query(
        "UPDATE bounty_profiles SET reroll_tokens = reroll_tokens - 1, difficulty = $2
         WHERE character_id = $1 AND reroll_tokens > 0",
    )
    .bind(id)
    .bind(difficulty as i64)
    .execute(&mut *transaction)
    .await?;
    if spent.rows_affected() != 1 {
        return Ok(Err(Refusal::NoTokens));
    }
    sqlx::query("DELETE FROM bounty_tasks WHERE character_id = $1")
        .bind(id)
        .execute(&mut *transaction)
        .await?;
    if !write_offers(&mut transaction, &candidates, id, difficulty).await? {
        return Ok(Err(Refusal::NoMonsters));
    }
    transaction.commit().await?;
    Ok(Ok(()))
}

/// Starts the offer in `slot`.
pub async fn pick(pool: &sqlx::PgPool, character_id: i32, slot: u32) -> Result<Result<(), Refusal>, Error> {
    let id = i64::from(character_id);
    let mut transaction = pool.begin().await?;
    let offer: Option<(i64, i64, i64, i64, i64)> = sqlx::query_as(
        "DELETE FROM bounty_offers WHERE character_id = $1 AND slot = $2
         RETURNING monster_type, kills, grade, points, experience",
    )
    .bind(id)
    .bind(i64::from(slot))
    .fetch_optional(&mut *transaction)
    .await?;
    let Some((monster_type, kills, grade, points, experience)) = offer else {
        return Ok(Err(Refusal::NoSuchOffer));
    };
    let started = sqlx::query(
        "INSERT INTO bounty_tasks (character_id, monster_type, kills_required, grade, points, experience)
         VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING",
    )
    .bind(id)
    .bind(monster_type)
    .bind(kills)
    .bind(grade)
    .bind(points)
    .bind(experience)
    .execute(&mut *transaction)
    .await?;
    if started.rows_affected() != 1 {
        return Ok(Err(Refusal::TaskUnderWay));
    }
    sqlx::query("DELETE FROM bounty_offers WHERE character_id = $1")
        .bind(id)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(Ok(()))
}

/// Takes the day's free token.
pub async fn claim_daily(pool: &sqlx::PgPool, character_id: i32) -> Result<Result<(), Refusal>, Error> {
    let board = board(pool, character_id).await?;
    if board.tokens >= MAX_TOKENS {
        return Ok(Err(Refusal::TokensFull));
    }
    let claimed = sqlx::query(
        "UPDATE bounty_profiles SET reroll_tokens = reroll_tokens + 1, daily_claimed_on = $2
         WHERE character_id = $1 AND daily_claimed_on < $2 AND reroll_tokens < $3",
    )
    .bind(i64::from(character_id))
    .bind(today())
    .bind(i64::from(MAX_TOKENS))
    .execute(pool)
    .await?;
    Ok(if claimed.rows_affected() == 1 { Ok(()) } else { Err(Refusal::DailyTaken) })
}

/// Raises a talisman bonus one level for its cost in bounty points.
pub async fn upgrade(pool: &sqlx::PgPool, character_id: i32, bonus: Bonus) -> Result<Result<u32, Refusal>, Error> {
    let board = board(pool, character_id).await?;
    let level = board.talisman[bonus as usize];
    if level >= MAX_TALISMAN_LEVEL {
        return Ok(Err(Refusal::MaxLevel));
    }
    let cost = upgrade_cost(level);
    // The level in the condition makes a second click on the same level fail.
    let column = bonus.column();
    let upgraded = sqlx::query(&format!(
        "UPDATE bounty_profiles SET points = points - $2, {column} = {column} + 1
         WHERE character_id = $1 AND points >= $2 AND {column} = $3"
    ))
    .bind(i64::from(character_id))
    .bind(i64::from(cost))
    .bind(i64::from(level))
    .execute(pool)
    .await?;
    Ok(if upgraded.rows_affected() == 1 {
        Ok(level + 1)
    } else {
        Err(Refusal::NotEnoughPoints(cost))
    })
}

/// Counts `kills` of `monster_type`; the kill that finishes the task pays its
/// reward and brings a free list of the same difficulty.
pub async fn record_kills(
    pool: &sqlx::PgPool,
    character_id: i32,
    monster_type: u32,
    kills: u32,
) -> Result<Progress, Error> {
    let id = i64::from(character_id);
    let counted: Option<(i64, i64)> = sqlx::query_as(
        "UPDATE bounty_tasks b SET kills_done = LEAST(b.kills_done + $3, b.kills_required)
         WHERE b.character_id = $1
           AND (SELECT name FROM monster_types WHERE id = b.monster_type)
             = (SELECT name FROM monster_types WHERE id = $2)
         RETURNING b.kills_done, b.kills_required",
    )
    .bind(id)
    .bind(i64::from(monster_type))
    .bind(i64::from(kills))
    .fetch_optional(pool)
    .await?;
    let Some((done, required)) = counted else {
        return Ok(Progress::None);
    };
    if done < required {
        return Ok(Progress::Counted {
            done: u32::try_from(done).unwrap_or_default(),
            required: u32::try_from(required).unwrap_or_default(),
        });
    }
    let candidates = candidates(pool).await?;
    let mut transaction = pool.begin().await?;
    // Only the kill whose delete takes the finished task pays for it.
    let finished: Option<(i64, i64, String)> = sqlx::query_as(
        "DELETE FROM bounty_tasks b WHERE b.character_id = $1 AND b.kills_done >= b.kills_required
         RETURNING b.points, b.experience,
                   COALESCE((SELECT t.name FROM monster_types t WHERE t.id = b.monster_type), '')",
    )
    .bind(id)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some((points, experience, monster_name)) = finished else {
        return Ok(Progress::None);
    };
    let (tokens, difficulty): (i64, i64) = sqlx::query_as(
        "UPDATE bounty_profiles
         SET points = points + $2, reroll_tokens = LEAST(reroll_tokens + 1, $3), completed = completed + 1
         WHERE character_id = $1
         RETURNING reroll_tokens, difficulty",
    )
    .bind(id)
    .bind(points)
    .bind(i64::from(MAX_TOKENS))
    .fetch_one(&mut *transaction)
    .await?;
    let difficulty =
        Difficulty::from_u32(u32::try_from(difficulty).unwrap_or_default()).unwrap_or(Difficulty::Beginner);
    write_offers(&mut transaction, &candidates, id, difficulty).await?;
    transaction.commit().await?;
    Ok(Progress::Completed(Reward {
        monster_name,
        points: u32::try_from(points).unwrap_or_default(),
        experience: u64::try_from(experience).unwrap_or_default(),
        tokens: u32::try_from(tokens).unwrap_or_default(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidates() -> Vec<Candidate> {
        (1..=12_u16)
            .map(|n| Candidate {
                monster_type: u32::from(n),
                name: format!("Monster {n}"),
                level: n * 10,
            })
            .collect()
    }

    async fn character(pool: &sqlx::PgPool) -> i32 {
        crate::packets::MsgRegister::build_character_with(
            "bounty-test".to_owned(),
            crate::packets::BodyType::MuscularMale,
            crate::packets::BaseClass::Trojan,
            1,
            1,
        )
        .expect("character")
        .save(pool)
        .await
        .expect("saved")
    }

    #[tokio::test]
    async fn a_bounty_from_the_board_to_its_reward() -> Result<(), Error> {
        let pool = tq_db::testing::pool().await;
        let id = character(&pool).await;

        let board = super::board(&pool, id).await?;
        assert_eq!((board.points, board.tokens, board.difficulty), (0, 3, Difficulty::Beginner));
        assert_eq!(board.offers.len(), 3, "a free Beginner list to start");
        assert!(board.offers.iter().all(|offer| !offer.monster_name.is_empty()));
        assert!(board.task.is_none() && board.daily_available);

        let chosen = board.offers[1].clone();
        assert_eq!(pick(&pool, id, 1).await?, Ok(()));
        assert_eq!(pick(&pool, id, 0).await?, Err(Refusal::NoSuchOffer), "the other offers are gone");
        let board = super::board(&pool, id).await?;
        let task = board.task.clone().expect("task under way");
        assert_eq!((task.monster_type, task.kills_done), (chosen.monster_type, 0));
        assert!(board.offers.is_empty());
        assert_eq!(board.talisman().map(|talisman| talisman.monster_name), Some(chosen.monster_name.clone()));

        assert_eq!(record_kills(&pool, id, 900, 1).await?, Progress::None, "a guard is not the bounty");
        assert_eq!(
            record_kills(&pool, id, chosen.monster_type, 2).await?,
            Progress::Counted { done: 2, required: task.kills }
        );
        let Progress::Completed(reward) = record_kills(&pool, id, chosen.monster_type, task.kills).await? else {
            panic!("the last kills finish it");
        };
        assert_eq!((reward.points, reward.experience, reward.tokens), (task.points, task.experience, 4));
        assert_eq!(record_kills(&pool, id, chosen.monster_type, 1).await?, Progress::None, "paid once");
        let board = super::board(&pool, id).await?;
        assert_eq!((board.points, board.completed, board.offers.len()), (task.points, 1, 3));

        // A token for a Master list.
        assert_eq!(reroll(&pool, id, Difficulty::Master).await?, Ok(()));
        let board = super::board(&pool, id).await?;
        assert_eq!((board.tokens, board.difficulty), (3, Difficulty::Master));

        assert_eq!(claim_daily(&pool, id).await?, Ok(()));
        assert_eq!(claim_daily(&pool, id).await?, Err(Refusal::DailyTaken));

        sqlx::query("UPDATE bounty_profiles SET points = 20 WHERE character_id = $1")
            .bind(i64::from(id))
            .execute(&pool)
            .await?;
        assert_eq!(upgrade(&pool, id, Bonus::Leech).await?, Ok(1));
        assert_eq!(upgrade(&pool, id, Bonus::Leech).await?, Err(Refusal::NotEnoughPoints(17)));
        let board = super::board(&pool, id).await?;
        assert_eq!((board.points, board.talisman), (15, [0, 1, 0, 0]));

        // 15 points: one preferred monster (10), then the avoided list is
        // short of points after one (5).
        let monster = board.offers.first().map(|offer| offer.monster_type).unwrap_or(1);
        let name = list_monster(&pool, id, monster, false).await?.expect("listed");
        assert_eq!(list_monster(&pool, id, monster, true).await?, Err(Refusal::AlreadyListed));
        assert_eq!(list_monster(&pool, id, u32::MAX, true).await?, Err(Refusal::NoSuchMonster));
        let board = super::board(&pool, id).await?;
        assert_eq!((board.points, board.lists.preferred.clone()), (5, vec![name.clone()]));
        assert_eq!(unlist(&pool, id, false, 0).await?, Ok(name));
        assert_eq!(unlist(&pool, id, false, 0).await?, Err(Refusal::NotListed));
        assert_eq!(super::board(&pool, id).await?.lists, Lists::default());

        for _ in 0..4 {
            reroll(&pool, id, Difficulty::Beginner).await?.expect("tokens left");
        }
        assert_eq!(reroll(&pool, id, Difficulty::Beginner).await?, Err(Refusal::NoTokens));
        Ok(())
    }

    #[test]
    fn offers_are_three_monsters_of_the_difficulty() {
        let offers = roll_offers(&candidates(), Difficulty::Adept, &Lists::default(), &mut (0..).map(|n| n * 7));
        assert_eq!(offers.len(), 3);
        let mut types: Vec<_> = offers.iter().map(|offer| offer.0.monster_type).collect();
        types.dedup();
        assert_eq!(types.len(), 3, "three different monsters");
        for (candidate, kills, grade, points, experience) in offers {
            assert!((20..=70).contains(&candidate.level));
            let (kills_percent, points_percent, _) = grade.percents();
            assert!((80 * kills_percent / 100..=190 * kills_percent / 100).contains(&kills));
            assert_eq!(points, (7 * points_percent).div_ceil(100));
            assert!(experience > 0);
        }
        assert!(roll_offers(&[], Difficulty::Master, &Lists::default(), &mut (0..)).is_empty());
    }

    #[test]
    fn lists_steer_the_offers() {
        // Adept draws from Monster 2..=7.
        let avoid = Lists {
            avoided: vec!["Monster 2".to_owned(), "Monster 3".to_owned(), "Monster 4".to_owned()],
            ..Lists::default()
        };
        for seed in 0..50_u32 {
            let offers = roll_offers(&candidates(), Difficulty::Adept, &avoid, &mut (seed..).map(|n| n * 13));
            assert_eq!(offers.len(), 3);
            assert!(offers.iter().all(|offer| !avoid.avoided.contains(&offer.0.name)));
        }
        // Avoiding every monster of a difficulty still brings a list.
        let all = Lists {
            avoided: (1..=12).map(|n| format!("Monster {n}")).collect(),
            ..Lists::default()
        };
        assert_eq!(roll_offers(&candidates(), Difficulty::Adept, &all, &mut (0..)).len(), 3);
        // A preferred monster takes the first offer four times in nine.
        let prefer = Lists {
            preferred: vec!["Monster 5".to_owned()],
            ..Lists::default()
        };
        let first = |roll: u32| roll_offers(&candidates(), Difficulty::Adept, &prefer, &mut [roll, 0, 0].into_iter())[0].0.name.clone();
        let hits = (0..9).filter(|roll| first(*roll) == "Monster 5").count();
        assert_eq!(hits, 4);
    }

    #[test]
    fn grades_pay_more_for_more_kills() {
        assert_eq!(Grade::roll(0), Grade::Normal);
        assert_eq!(Grade::roll(69), Grade::Normal);
        assert_eq!(Grade::roll(70), Grade::Silver);
        assert_eq!(Grade::roll(92), Grade::Gold);
        let gold = roll_offers(&candidates()[..1], Difficulty::Beginner, &Lists::default(), &mut [0, 95, 0].into_iter());
        assert_eq!((gold[0].1, gold[0].2, gold[0].3), (100, Grade::Gold, 8));
    }

    #[test]
    fn talisman_levels_slow_down_past_their_first_twenty() {
        assert_eq!(Bonus::Damage.basis_points(0), 0);
        assert_eq!(Bonus::Damage.basis_points(20), 1_000);
        assert_eq!(Bonus::Damage.basis_points(24), 1_100);
        assert_eq!(Bonus::DoubleKill.basis_points(20), 2_000);
        assert_eq!(Bonus::DoubleKill.basis_points(99), 3_000, "capped at the highest level");
        assert_eq!((upgrade_cost(0), upgrade_cost(1), upgrade_cost(4)), (5, 17, 53));
    }

    #[test]
    fn the_talisman_works_only_against_the_task_monster() {
        let talisman = Talisman {
            monster_name: "Robin".to_owned(),
            damage_bp: 1_000,
            leech_bp: 500,
            loot_bp: 2_500,
            double_kill_bp: 2_000,
        };
        assert_eq!(talisman.boost("Robin", 1_000), 1_100);
        assert_eq!(talisman.boost("Pheasant", 1_000), 1_000);
        assert_eq!(talisman.leech("Robin", 1_000), 50);
        assert_eq!(talisman.leech("Pheasant", 1_000), 0);
        assert!(talisman.extra_loot("Robin", 2_499) && !talisman.extra_loot("Robin", 2_500));
        assert!(talisman.double_kill("Robin", 1_999) && !talisman.double_kill("Pheasant", 0));
        assert!(!Talisman::default().hunts(""), "no bounty, no bonus");
    }
}
