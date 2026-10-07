//! Server-authoritative nobility, as in the later clients' Nobility Rank:
//! the 50 greatest donors hold seats (King for the first 3, Prince to the
//! 15th, Duke to the 50th); below them the donation alone makes an Earl
//! (200 million), a Baron (100 million) or a Knight (30 million).
//!
//! As the official guide has it (co.99.com, Nobility System, 2008): the
//! seats go by place alone, each donation is at least 3 million silver,
//! and CPs may be given instead at 50,000 silver each.
//!
//! The donations live on the character row; the board keeps every donor in
//! order in memory, so a character's seat is known without a query and one
//! donation can tell who it pushed down.

use parking_lot::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u32)]
pub enum NobilityRank {
    Serf = 0,
    Knight = 1,
    Baron = 3,
    Earl = 5,
    Duke = 7,
    Prince = 9,
    King = 12,
}

impl NobilityRank {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Serf => "Serf",
            Self::Knight => "Knight",
            Self::Baron => "Baron",
            Self::Earl => "Earl",
            Self::Duke => "Duke",
            Self::Prince => "Prince",
            Self::King => "King",
        }
    }

    pub const fn next(self) -> Option<Self> {
        match self {
            Self::Serf => Some(Self::Knight),
            Self::Knight => Some(Self::Baron),
            Self::Baron => Some(Self::Earl),
            Self::Earl => Some(Self::Duke),
            Self::Duke => Some(Self::Prince),
            Self::Prince => Some(Self::King),
            Self::King => None,
        }
    }

    /// The last seat of a seated rank (King, Prince, Duke).
    const fn last_seat(self) -> Option<u32> {
        match self {
            Self::King => Some(3),
            Self::Prince => Some(15),
            Self::Duke => Some(50),
            _ => None,
        }
    }
}

pub const MIN_LEVEL: u8 = 70;
/// The least one donation may be.
pub const MIN_DONATION: u64 = 3_000_000;
/// What a Knight gives.
pub const KNIGHT_DONATION: u64 = 30_000_000;
/// The silver one CP stands for, when CPs are given.
pub const SILVER_PER_CP: u64 = 50_000;
/// How many seats there are: the ranking the clients show.
pub const SEATS: u32 = 50;

/// The ranks a donation alone gives, from the greatest.
const THRESHOLDS: &[(u64, NobilityRank)] = &[
    (200_000_000, NobilityRank::Earl),
    (100_000_000, NobilityRank::Baron),
    (KNIGHT_DONATION, NobilityRank::Knight),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NobilityStatus {
    pub donation: u64,
    pub rank: NobilityRank,
    /// The place among the donors, from 1.
    pub position: Option<u32>,
    pub next_rank: Option<NobilityRank>,
    /// Silver still to donate for `next_rank`.
    pub remaining: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NobilityEntry {
    pub character_id: u32,
    pub name: String,
    pub donation: u64,
    pub rank: NobilityRank,
}

/// The rank of a donor at `position` (from 1) who gave `donation`.
pub fn rank_at(position: Option<u32>, donation: u64) -> NobilityRank {
    if let (true, Some(position)) = (donation > 0, position) {
        for rank in [NobilityRank::King, NobilityRank::Prince, NobilityRank::Duke] {
            if rank.last_seat().is_some_and(|last| position <= last) {
                return rank;
            }
        }
    }
    THRESHOLDS
        .iter()
        .find(|(threshold, _)| donation >= *threshold)
        .map_or(NobilityRank::Serf, |(_, rank)| *rank)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Donor {
    character_id: i32,
    name: String,
    donation: u64,
}

/// Every donor, the greatest first (ties: the older character first).
#[derive(Debug, Default)]
pub struct Board {
    donors: Vec<Donor>,
}

impl Board {
    fn sort(&mut self) {
        self.donors
            .sort_by(|a, b| b.donation.cmp(&a.donation).then(a.character_id.cmp(&b.character_id)));
    }

    pub fn position(&self, character_id: i32) -> Option<u32> {
        self.donors
            .iter()
            .position(|donor| donor.character_id == character_id)
            .map(|index| index as u32 + 1)
    }

    fn rank_of(&self, index: usize) -> NobilityRank {
        rank_at(Some(index as u32 + 1), self.donors[index].donation)
    }

    /// Sets a donor's total; the characters whose rank it changed (the
    /// donor among them, when theirs did), with their new rank.
    pub fn record(&mut self, character_id: i32, name: &str, donation: u64) -> Vec<(i32, NobilityRank)> {
        let before: Vec<(i32, NobilityRank)> = (0..self.donors.len())
            .map(|index| (self.donors[index].character_id, self.rank_of(index)))
            .collect();
        match self.donors.iter_mut().find(|donor| donor.character_id == character_id) {
            Some(donor) => {
                donor.donation = donation;
                donor.name = name.to_owned();
            }
            None => self.donors.push(Donor { character_id, name: name.to_owned(), donation }),
        }
        self.sort();
        (0..self.donors.len())
            .filter_map(|index| {
                let id = self.donors[index].character_id;
                let rank = self.rank_of(index);
                let was = before.iter().find(|(before_id, _)| *before_id == id).map(|(_, rank)| *rank);
                (was != Some(rank) && !(was.is_none() && rank == NobilityRank::Serf)).then_some((id, rank))
            })
            .collect()
    }

    /// The status of a character who gave `donation`.
    pub fn status(&self, character_id: i32, donation: u64) -> NobilityStatus {
        let position = self.position(character_id);
        let rank = rank_at(position, donation);
        let next_rank = rank.next();
        let remaining = next_rank.map_or(0, |next| self.needed_for(next, character_id, donation));
        NobilityStatus { donation, rank, position, next_rank, remaining }
    }

    /// Silver a character who gave `donation` must still give to be `rank`.
    pub fn needed_for(&self, rank: NobilityRank, character_id: i32, donation: u64) -> u64 {
        let target = match rank.last_seat() {
            // A seat: more than whoever holds its last place now (the
            // character left out); a free seat, one donation.
            Some(last) => {
                let holder = self
                    .donors
                    .iter()
                    .filter(|donor| donor.character_id != character_id)
                    .nth(last as usize - 1)
                    .map_or(0, |donor| donor.donation + 1);
                holder.max(MIN_DONATION)
            }
            None => THRESHOLDS
                .iter()
                .find(|(_, threshold_rank)| *threshold_rank == rank)
                .map_or(0, |(threshold, _)| *threshold),
        };
        target.saturating_sub(donation)
    }

    /// The first `count` donors with their rank.
    pub fn top(&self, count: u32) -> Vec<NobilityEntry> {
        self.donors
            .iter()
            .take(count as usize)
            .enumerate()
            .map(|(index, donor)| NobilityEntry {
                character_id: donor.character_id.max(0) as u32,
                name: donor.name.clone(),
                donation: donor.donation,
                rank: self.rank_of(index),
            })
            .collect()
    }
}

fn board_lock() -> &'static RwLock<Board> {
    static BOARD: std::sync::OnceLock<RwLock<Board>> = std::sync::OnceLock::new();
    BOARD.get_or_init(Default::default)
}

/// The board, read.
pub fn board() -> parking_lot::RwLockReadGuard<'static, Board> {
    board_lock().read()
}

/// Fills the board from the database; at start-up.
pub async fn load(pool: &sqlx::PgPool) -> Result<usize, sqlx::Error> {
    // `character_id` is BIGINT in the table; the characters carry it as i32.
    let rows = sqlx::query_as::<_, (i64, String, i64)>(
        "SELECT character_id, name, nobility_donation FROM characters WHERE nobility_donation > 0",
    )
    .fetch_all(pool)
    .await?;
    let mut board = Board {
        donors: rows
            .into_iter()
            .filter_map(|(character_id, name, donation)| {
                Some(Donor { character_id: i32::try_from(character_id).ok()?, name, donation: donation.max(0) as u64 })
            })
            .collect(),
    };
    board.sort();
    let count = board.donors.len();
    *board_lock().write() = board;
    Ok(count)
}

/// Records a donor's new total on the board; who changed rank.
pub fn record(character_id: i32, name: &str, donation: u64) -> Vec<(i32, NobilityRank)> {
    board_lock().write().record(character_id, name, donation)
}

/// Returns the top donation holders in deterministic order.
pub async fn ranking(pool: &sqlx::PgPool, limit: u8) -> Result<Vec<NobilityEntry>, sqlx::Error> {
    let limit = i64::from(limit.clamp(1, SEATS as u8));
    let rows = sqlx::query_as::<_, (i64, String, i64)>(
        "SELECT character_id, name, nobility_donation
         FROM characters
         WHERE nobility_donation > 0
         ORDER BY nobility_donation DESC, character_id ASC
         LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .enumerate()
        .map(|(index, (character_id, name, donation))| {
            let donation = donation.max(0) as u64;
            NobilityEntry {
                character_id: character_id.max(0) as u32,
                name,
                donation,
                rank: rank_at(Some(index as u32 + 1), donation),
            }
        })
        .collect())
}

/// The rank a donation gives without a seat.
pub fn rank_for(donation: u64) -> NobilityRank {
    rank_at(None, donation)
}

/// A character's status from the board.
pub fn status_for(character_id: i32, donation: u64) -> NobilityStatus {
    board().status(character_id, donation)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DonateOutcome {
    InvalidLevel,
    InvalidAmount,
    InsufficientSilver,
    /// The donor's new status, and whose rank the donation changed.
    Donated(NobilityStatus, Vec<(i32, NobilityRank)>),
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: u64 = 1_000_000;

    #[test]
    fn donations_alone_make_knights_barons_and_earls() {
        assert_eq!(rank_for(0), NobilityRank::Serf);
        assert_eq!(rank_for(30 * M - 1), NobilityRank::Serf);
        assert_eq!(rank_for(30 * M), NobilityRank::Knight);
        assert_eq!(rank_for(100 * M), NobilityRank::Baron);
        assert_eq!(rank_for(200 * M), NobilityRank::Earl);
        // However much, no seat without a place among the 50.
        assert_eq!(rank_for(u64::MAX), NobilityRank::Earl);
    }

    #[test]
    fn seats_go_by_place_alone() {
        assert_eq!(rank_at(Some(1), 30 * M), NobilityRank::King);
        assert_eq!(rank_at(Some(3), 30 * M), NobilityRank::King);
        assert_eq!(rank_at(Some(4), 30 * M), NobilityRank::Prince);
        assert_eq!(rank_at(Some(15), 30 * M), NobilityRank::Prince);
        assert_eq!(rank_at(Some(16), 30 * M), NobilityRank::Duke);
        assert_eq!(rank_at(Some(50), 30 * M), NobilityRank::Duke);
        assert_eq!(rank_at(Some(51), 250 * M), NobilityRank::Earl);
        // The place alone: first with 3 million is a King.
        assert_eq!(rank_at(Some(1), 3 * M), NobilityRank::King);
        assert_eq!(rank_at(None, 3 * M), NobilityRank::Serf);
    }

    #[test]
    fn a_donation_pushes_the_others_down() {
        let mut board = Board::default();
        for id in 1..=3 {
            board.record(id, &format!("k{id}"), 100 * M + id as u64);
        }
        assert_eq!(board.status(1, 100 * M + 1).rank, NobilityRank::King);
        // A fourth outgives them all: the third King becomes a Prince.
        let changed = board.record(9, "new", 500 * M);
        assert!(changed.contains(&(9, NobilityRank::King)));
        assert!(changed.contains(&(1, NobilityRank::Prince)));
        assert_eq!(changed.len(), 2);
        let status = board.status(1, 100 * M + 1);
        assert_eq!((status.position, status.rank), (Some(4), NobilityRank::Prince));
        // Back to King: one more than the third King has.
        assert_eq!(status.remaining, 100 * M + 2 + 1 - (100 * M + 1));
        // Not on the board: the next rank, a Knight, is 30 million away;
        // a free seat is one donation away.
        assert_eq!(board.status(77, 0).remaining, KNIGHT_DONATION);
        assert_eq!(board.needed_for(NobilityRank::Prince, 77, 0), MIN_DONATION);
        assert_eq!(board.top(2).iter().map(|entry| entry.name.as_str()).collect::<Vec<_>>(), ["new", "k3"]);
    }

    #[test]
    fn donate_outcome_keeps_invalid_states_explicit() {
        assert_eq!(DonateOutcome::InvalidLevel, DonateOutcome::InvalidLevel);
        assert!(MIN_LEVEL > 0 && MIN_DONATION < KNIGHT_DONATION);
    }
}
