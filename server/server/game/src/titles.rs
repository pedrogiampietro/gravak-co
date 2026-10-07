use crate::nobility::{NobilityRank, NobilityStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TitleDefinition {
    pub id: u32,
    pub name: &'static str,
    pub required_level: u8,
    pub required_rank: NobilityRank,
}

pub const NONE: u32 = 0;

const TITLES: &[TitleDefinition] = &[
    TitleDefinition {
        id: 1,
        name: "Novice",
        required_level: 1,
        required_rank: NobilityRank::Serf,
    },
    TitleDefinition {
        id: 2,
        name: "Veteran",
        required_level: 30,
        required_rank: NobilityRank::Serf,
    },
    TitleDefinition {
        id: 3,
        name: "Knight",
        required_level: 70,
        required_rank: NobilityRank::Knight,
    },
    TitleDefinition {
        id: 4,
        name: "Baron",
        required_level: 70,
        required_rank: NobilityRank::Baron,
    },
    TitleDefinition {
        id: 5,
        name: "Duke",
        required_level: 70,
        required_rank: NobilityRank::Duke,
    },
];

pub fn definition(id: u32) -> Option<TitleDefinition> {
    TITLES.iter().copied().find(|title| title.id == id)
}

pub fn is_unlocked(title: TitleDefinition, level: u8, status: NobilityStatus) -> bool {
    level >= title.required_level && status.rank >= title.required_rank
}

pub fn name(id: u32) -> &'static str {
    definition(id).map_or("Nenhum", |title| title.name)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquipOutcome {
    Equipped(u32),
    UnknownTitle,
    Locked,
}

#[cfg(test)]
mod tests {
    use super::{definition, is_unlocked, name, EquipOutcome, NONE};
    use crate::nobility::{status_for, NobilityRank};

    #[test]
    fn catalog_validates_level_and_nobility_requirements() {
        let novice = definition(1).expect("novice title should exist");
        assert!(is_unlocked(novice, 1, status_for(-1, 0)));
        let knight = definition(3).expect("knight title should exist");
        assert!(!is_unlocked(knight, 70, status_for(-1, 0)));
        assert!(is_unlocked(knight, 70, status_for(-1, 30_000_000)));
        assert_eq!(knight.required_rank, NobilityRank::Knight);
    }

    #[test]
    fn unknown_and_none_titles_have_safe_labels() {
        assert_eq!(name(NONE), "Nenhum");
        assert_eq!(name(999), "Nenhum");
        assert_eq!(EquipOutcome::UnknownTitle, EquipOutcome::UnknownTitle);
    }
}
