// Source: @digitalm1nd on x.com / _dek on Discord — Preservation Conquer project — https://discord.gg/CvKPXEHYRY
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Action {
    #[default]
    Idle,
    WalkLeft,
    WalkRight,
    RunLeft,
    RunRight,
    Jump,
    Attack,
    Hit,
    /// Conquer's action 220 (Kneel), used by the original client while
    /// collecting an item from the ground.
    Pickup,
}

impl Action {
    pub const ALL: [Self; 9] = [
        Self::Idle,
        Self::WalkLeft,
        Self::WalkRight,
        Self::RunLeft,
        Self::RunRight,
        Self::Jump,
        Self::Attack,
        Self::Hit,
        Self::Pickup,
    ];

    pub const fn index(self) -> usize {
        match self {
            Self::Idle => 0,
            Self::WalkLeft => 1,
            Self::WalkRight => 2,
            Self::RunLeft => 3,
            Self::RunRight => 4,
            Self::Jump => 5,
            Self::Attack => 6,
            Self::Hit => 7,
            Self::Pickup => 8,
        }
    }

    pub const fn catalog_id(self) -> u32 {
        match self {
            Self::Idle => 100,
            Self::WalkLeft => 110,
            Self::WalkRight => 111,
            Self::RunLeft => 120,
            Self::RunRight => 121,
            Self::Jump => 130,
            Self::Attack => 140,
            // Conquer's 150/160/170/180 entries are emotes (laugh, angry,
            // sad, excited), not a damage reaction. Keep the hit state on
            // the neutral motion until a dedicated hurt asset is available.
            Self::Hit => 100,
            // Action.dat/3dmotion.ini in the original 5065 client names this
            // motion Kneel (220); it is the pickup pose.
            Self::Pickup => 220,
        }
    }

    pub const fn paired_action(self) -> Option<Self> {
        match self {
            Self::WalkRight => Some(Self::WalkLeft),
            Self::RunRight => Some(Self::RunLeft),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Action;

    #[test]
    fn actions_keep_their_catalog_and_storage_order() {
        assert_eq!(Action::ALL.map(Action::index), [0, 1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(
            Action::ALL.map(Action::catalog_id),
            [100, 110, 111, 120, 121, 130, 140, 100, 220]
        );
    }
}
