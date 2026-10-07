//! The Top titles: the highest of each profession (rebirths, then level,
//! then experience, as the Highscores rank them) is Top Trojan, Top
//! Warrior, Top Archer or Top Taoist. The clients show it circling the
//! hero: `MsgPlayer` carries the flags to those who see them, `MsgNobility`
//! to the hero.

use crate::highscores::{Category, Profession};
use crate::State;
use parking_lot::Mutex;
use std::collections::HashMap;

pub const TROJAN: u32 = 1;
pub const WARRIOR: u32 = 2;
pub const ARCHER: u32 = 4;
pub const TAOIST: u32 = 8;

const PROFESSIONS: [(Profession, u32); 4] = [
    (Profession::Trojan, TROJAN),
    (Profession::Warrior, WARRIOR),
    (Profession::Archer, ARCHER),
    (Profession::Taoist, TAOIST),
];

/// How often the tops are looked at again, in ticks of half a second.
const EVERY_TICKS: u32 = 120;

#[derive(Default)]
struct Book {
    /// Each top's flags, by database character id.
    tops: HashMap<i32, u32>,
    ticks: u32,
}

fn book() -> &'static Mutex<Book> {
    static BOOK: std::sync::OnceLock<Mutex<Book>> = std::sync::OnceLock::new();
    BOOK.get_or_init(|| Mutex::new(Book { ticks: EVERY_TICKS, ..Default::default() }))
}

/// The Top flags a character holds.
pub fn flags_of(database_id: i32) -> u32 {
    book().lock().tops.get(&database_id).copied().unwrap_or(0)
}

/// Who holds what from the first of each profession's ranking.
fn tops_from(firsts: &[(u32, Option<i32>)]) -> HashMap<i32, u32> {
    let mut tops = HashMap::new();
    for (flag, holder) in firsts {
        if let Some(id) = holder {
            *tops.entry(*id).or_insert(0) |= flag;
        }
    }
    tops
}

/// Looks at the rankings now and then; whoever won or lost a title is told,
/// and so are those who see them.
pub async fn tick(state: &State) {
    {
        let mut book = book().lock();
        book.ticks += 1;
        if book.ticks < EVERY_TICKS {
            return;
        }
        book.ticks = 0;
    }
    let mut firsts = Vec::new();
    for (profession, flag) in PROFESSIONS {
        let page = state.highscores().page(state.pool(), Category::Level, profession, Some(0), 0).await;
        let holder = match page {
            Ok(page) => page.rows.first().map(|(_, entry)| entry.character_id as i32),
            Err(error) => {
                tracing::warn!(%error, "top titles: ranking unavailable");
                return;
            },
        };
        firsts.push((flag, holder));
    }
    let tops = tops_from(&firsts);
    let before = std::mem::replace(&mut book().lock().tops, tops.clone());
    for entity in state.entities() {
        let Some(character) = entity.as_character() else {
            continue;
        };
        let id = character.database_id();
        if before.get(&id) == tops.get(&id) {
            continue;
        }
        let _ = character.owner().send(crate::packets::MsgNobility::from(character)).await;
        if let Ok(screen) = character.try_screen() {
            let _ = screen.send_message(crate::packets::MsgPlayer::from(character)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_hero_can_top_more_than_one_ranking() {
        let tops = tops_from(&[(TROJAN, Some(7)), (WARRIOR, Some(9)), (ARCHER, None), (TAOIST, Some(7))]);
        assert_eq!(tops.get(&7), Some(&(TROJAN | TAOIST)));
        assert_eq!(tops.get(&9), Some(&WARRIOR));
        assert_eq!(tops.len(), 2);
    }
}
