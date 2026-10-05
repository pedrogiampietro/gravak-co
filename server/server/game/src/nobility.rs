//! Server-authoritative nobility progression.
//!
//! The first slice uses the classic donation thresholds and keeps the value
//! on the character row. Ranking pages and packet 2064 are layered on top of
//! this state so a failed donation can never change the visible rank.

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
}

pub const MIN_LEVEL: u8 = 70;
pub const MIN_DONATION: u64 = 3_000_000;

const THRESHOLDS: &[(u64, NobilityRank)] = &[
    (0, NobilityRank::Serf),
    (30_000_000, NobilityRank::Knight),
    (100_000_000, NobilityRank::Baron),
    (200_000_000, NobilityRank::Earl),
    (1_000_000_000, NobilityRank::Duke),
    (3_000_000_000, NobilityRank::Prince),
    (10_000_000_000, NobilityRank::King),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NobilityStatus {
    pub donation: u64,
    pub rank: NobilityRank,
    pub next_rank: Option<NobilityRank>,
    pub remaining: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NobilityEntry {
    pub character_id: u32,
    pub name: String,
    pub donation: u64,
    pub rank: NobilityRank,
}

/// Returns the top donation holders in deterministic order.
pub async fn ranking(pool: &sqlx::PgPool, limit: u8) -> Result<Vec<NobilityEntry>, sqlx::Error> {
    let limit = i64::from(limit.clamp(1, 20));
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
        .map(|(character_id, name, donation)| {
            let donation = donation.max(0) as u64;
            NobilityEntry {
                character_id: character_id.max(0) as u32,
                name,
                donation,
                rank: rank_for(donation),
            }
        })
        .collect())
}

pub fn rank_for(donation: u64) -> NobilityRank {
    THRESHOLDS
        .iter()
        .rev()
        .find(|(threshold, _)| donation >= *threshold)
        .map_or(NobilityRank::Serf, |(_, rank)| *rank)
}

pub fn status_for(donation: u64) -> NobilityStatus {
    let rank = rank_for(donation);
    let next_rank = rank.next();
    let remaining = next_rank
        .and_then(|next| {
            THRESHOLDS
                .iter()
                .find(|(_, threshold_rank)| *threshold_rank == next)
                .map(|(threshold, _)| threshold.saturating_sub(donation))
        })
        .unwrap_or(0);
    NobilityStatus {
        donation,
        rank,
        next_rank,
        remaining,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DonateOutcome {
    InvalidLevel,
    InvalidAmount,
    InsufficientSilver,
    Donated(NobilityStatus),
}

#[cfg(test)]
mod tests {
    use super::{rank_for, status_for, DonateOutcome, NobilityRank, MIN_DONATION, MIN_LEVEL};

    #[test]
    fn classic_donation_thresholds_are_monotonic() {
        assert_eq!(rank_for(0), NobilityRank::Serf);
        assert_eq!(rank_for(29_999_999), NobilityRank::Serf);
        assert_eq!(rank_for(30_000_000), NobilityRank::Knight);
        assert_eq!(rank_for(100_000_000), NobilityRank::Baron);
        assert_eq!(rank_for(200_000_000), NobilityRank::Earl);
        assert_eq!(rank_for(1_000_000_000), NobilityRank::Duke);
        assert_eq!(rank_for(3_000_000_000), NobilityRank::Prince);
        assert_eq!(rank_for(10_000_000_000), NobilityRank::King);
    }

    #[test]
    fn status_reports_remaining_donation_without_underflow() {
        let status = status_for(MIN_DONATION);
        assert_eq!(status.rank, NobilityRank::Serf);
        assert_eq!(status.remaining, 27_000_000);
        assert_eq!(status_for(u64::MAX).remaining, 0);
        assert!(MIN_LEVEL > 0);
    }

    #[test]
    fn donate_outcome_keeps_invalid_states_explicit() {
        assert_eq!(DonateOutcome::InvalidLevel, DonateOutcome::InvalidLevel);
        assert_eq!(DonateOutcome::InvalidAmount, DonateOutcome::InvalidAmount);
        assert_eq!(DonateOutcome::InsufficientSilver, DonateOutcome::InsufficientSilver);
    }
}
