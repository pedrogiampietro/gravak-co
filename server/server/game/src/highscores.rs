//! Highscores: every character ranked by a category, filtered by
//! profession, ten to a page. The rankings are worked out from the
//! database at most every ten minutes and kept, so paging is free and the
//! window can say when they were last updated.

use std::collections::HashMap;
use std::time::Duration;

use parking_lot::Mutex;

use crate::Error;

/// Rows a page shows.
pub const PAGE_SIZE: usize = 10;
/// How long a ranking is kept before it is worked out again.
const REFRESH: Duration = Duration::from_secs(600);

/// What characters are ranked by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    /// Rebirths, then level, then experience.
    Level = 0,
    /// Bounty tasks completed.
    Bounties = 1,
    /// Silver donated for nobility.
    Nobility = 2,
    /// Days in a row of daily rewards, while the streak lives.
    DailyStreak = 3,
}

impl Category {
    pub const ALL: [Category; 4] = [Self::Level, Self::Bounties, Self::Nobility, Self::DailyStreak];

    pub fn from_id(id: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|category| *category as u32 == id)
    }

    /// The ranked characters, best first: id, name, class, level,
    /// rebirths, look (body and face, as `MsgPlayer` has it), value.
    fn query(self) -> &'static str {
        match self {
            Self::Level => {
                "SELECT character_id, name, current_class, level, rebirths, mesh + avatar * 10000, level
                 FROM characters ORDER BY rebirths DESC, level DESC, experience DESC, character_id"
            },
            Self::Bounties => {
                "SELECT c.character_id, c.name, c.current_class, c.level, c.rebirths, c.mesh + c.avatar * 10000, b.completed
                 FROM characters c JOIN bounty_profiles b ON b.character_id = c.character_id
                 WHERE b.completed > 0 ORDER BY b.completed DESC, c.level DESC, c.character_id"
            },
            Self::Nobility => {
                "SELECT character_id, name, current_class, level, rebirths, mesh + avatar * 10000, nobility_donation
                 FROM characters WHERE nobility_donation > 0
                 ORDER BY nobility_donation DESC, character_id"
            },
            Self::DailyStreak => {
                "SELECT c.character_id, c.name, c.current_class, c.level, c.rebirths, c.mesh + c.avatar * 10000, d.streak
                 FROM characters c JOIN daily_rewards d ON d.character_id = c.character_id
                 WHERE d.streak > 0 AND d.last_day >= $1 - 1
                 ORDER BY d.streak DESC, d.last_day DESC, c.character_id"
            },
        }
    }
}

/// The professions a ranking can be narrowed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Profession {
    All = 0,
    Trojan = 1,
    Warrior = 2,
    Archer = 3,
    Taoist = 4,
}

impl Profession {
    pub fn from_id(id: u32) -> Self {
        match id {
            1 => Self::Trojan,
            2 => Self::Warrior,
            3 => Self::Archer,
            4 => Self::Taoist,
            _ => Self::All,
        }
    }

    pub fn holds(self, class: u32) -> bool {
        match self {
            Self::All => true,
            Self::Trojan => (10..=15).contains(&class),
            Self::Warrior => (20..=25).contains(&class),
            Self::Archer => (40..=45).contains(&class),
            Self::Taoist => (100..=145).contains(&class),
        }
    }
}

/// One ranked character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub character_id: u32,
    pub name: String,
    pub class: u32,
    pub level: u32,
    pub rebirths: u32,
    pub mesh: u32,
    pub value: u64,
}

/// A ranking and when it was worked out (seconds since 1970).
#[derive(Debug, Clone, Default)]
struct Ranking {
    entries: Vec<Entry>,
    updated_at: u64,
}

/// The rankings, kept between refreshes.
#[derive(Debug, Default)]
pub struct Highscores {
    rankings: Mutex<HashMap<Category, Ranking>>,
}

/// One page of a ranking, as the window shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub page: usize,
    pub pages: usize,
    /// The page's rows with their ranks.
    pub rows: Vec<(usize, Entry)>,
    /// The first three, for the podium.
    pub podium: Vec<(usize, Entry)>,
    /// The asker's rank and value, when ranked.
    pub own: Option<(usize, u64)>,
    pub updated_at: u64,
}

impl Highscores {
    /// The page `page` of `category` narrowed to `profession`; the asker's
    /// own page when `page` is `None`.
    pub async fn page(
        &self,
        pool: &sqlx::PgPool,
        category: Category,
        profession: Profession,
        page: Option<usize>,
        asker: u32,
    ) -> Result<Page, Error> {
        let now = crate::store::now_secs();
        let fresh = self
            .rankings
            .lock()
            .get(&category)
            .filter(|ranking| now < ranking.updated_at + REFRESH.as_secs())
            .cloned();
        let ranking = match fresh {
            Some(ranking) => ranking,
            None => {
                let ranking = Ranking { entries: load(pool, category).await?, updated_at: now };
                self.rankings.lock().insert(category, ranking.clone());
                ranking
            },
        };
        Ok(paginate(&ranking.entries, profession, page, asker, ranking.updated_at))
    }
}

async fn load(pool: &sqlx::PgPool, category: Category) -> Result<Vec<Entry>, Error> {
    let mut query = sqlx::query_as::<_, (i64, String, i64, i64, i64, i64, i64)>(category.query());
    if category == Category::DailyStreak {
        query = query.bind(crate::daily_rewards::today());
    }
    let rows = query.fetch_all(pool).await?;
    Ok(rows
        .into_iter()
        .map(|(id, name, class, level, rebirths, mesh, value)| Entry {
            character_id: u32::try_from(id).unwrap_or_default(),
            name,
            class: u32::try_from(class).unwrap_or_default(),
            level: u32::try_from(level).unwrap_or_default(),
            rebirths: u32::try_from(rebirths).unwrap_or_default(),
            mesh: u32::try_from(mesh).unwrap_or_default(),
            value: u64::try_from(value).unwrap_or_default(),
        })
        .collect())
}

/// Narrows `entries` to `profession` and cuts out one page.
fn paginate(entries: &[Entry], profession: Profession, page: Option<usize>, asker: u32, updated_at: u64) -> Page {
    let ranked: Vec<(usize, &Entry)> = entries
        .iter()
        .filter(|entry| profession.holds(entry.class))
        .enumerate()
        .map(|(index, entry)| (index + 1, entry))
        .collect();
    let pages = ranked.len().div_ceil(PAGE_SIZE).max(1);
    let own = ranked.iter().find(|(_, entry)| entry.character_id == asker).map(|(rank, entry)| (*rank, entry.value));
    let page = page
        .unwrap_or_else(|| own.map_or(0, |(rank, _)| (rank - 1) / PAGE_SIZE))
        .min(pages - 1);
    let rows = ranked
        .iter()
        .skip(page * PAGE_SIZE)
        .take(PAGE_SIZE)
        .map(|(rank, entry)| (*rank, (*entry).clone()))
        .collect();
    let podium = ranked.iter().take(3).map(|(rank, entry)| (*rank, (*entry).clone())).collect();
    Page { page, pages, rows, podium, own, updated_at }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: u32, class: u32, value: u64) -> Entry {
        Entry { character_id: id, name: format!("Hero{id}"), class, level: 100, rebirths: 0, mesh: 1003, value }
    }

    #[test]
    fn pages_hold_ten_ranks_of_the_profession() {
        let entries: Vec<Entry> = (1..=25).map(|id| entry(id, if id % 2 == 0 { 11 } else { 21 }, 100 - id as u64)).collect();
        let all = paginate(&entries, Profession::All, Some(1), 0, 0);
        assert_eq!((all.page, all.pages, all.rows.len()), (1, 3, 10));
        assert_eq!(all.rows[0].0, 11, "the second page starts at rank 11");
        let trojans = paginate(&entries, Profession::Trojan, Some(0), 0, 0);
        assert_eq!(trojans.pages, 2, "twelve Trojans");
        assert_eq!(trojans.rows[0], (1, entry(2, 11, 98)), "ranked among Trojans");
        assert_eq!(trojans.podium.len(), 3);
    }

    #[test]
    fn the_asker_finds_their_own_page() {
        let entries: Vec<Entry> = (1..=30).map(|id| entry(id, 11, 100)).collect();
        let mine = paginate(&entries, Profession::All, None, 24, 0);
        assert_eq!(mine.own, Some((24, 100)));
        assert_eq!(mine.page, 2, "rank 24 is on the third page");
        let past = paginate(&entries, Profession::All, Some(99), 0, 0);
        assert_eq!(past.page, 2, "past the end: the last page");
        let empty = paginate(&[], Profession::All, None, 1, 0);
        assert_eq!((empty.page, empty.pages, empty.own), (0, 1, None));
    }

    #[tokio::test]
    async fn every_category_reads_from_the_database() -> Result<(), Error> {
        let pool = tq_db::testing::pool().await;
        for category in Category::ALL {
            load(&pool, category).await?;
        }
        Ok(())
    }
}
