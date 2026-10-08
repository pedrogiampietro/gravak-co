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
    /// `3dmotion.ini` 330: falling dead; the last frame is the corpse (401,
    /// once used here, is the blow: `140` points at `401.c3`).
    Die,
    /// `3dmotion.ini` 250: sitting down; the last frame is held.
    Sit,
    /// The Action panel's emotes (`Control.Ani` `ButtonA<n>`): Cool 230,
    /// Kneel 210, Sad 170, Happy 150, Angry 160, Lie 270, Dance 1, Wave
    /// 190, Bow 200.
    Cool,
    Kneel,
    Sad,
    Happy,
    Angry,
    Lie,
    Dance,
    Wave,
    Bow,
    /// Flying (the Fly skill): `Conquer.exe` gives a flier motion 510.
    Fly,
    /// Casting a magic (Thunder, Cure, Stigma...): MagicType's motion 903.
    Cast,
    /// The swept blow of Hercules, WideStrike, SpeedLightning, Snow and
    /// the like: MagicType's motion 403.
    Sweep,
}

impl Action {
    pub const COUNT: usize = 23;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Idle,
        Self::WalkLeft,
        Self::WalkRight,
        Self::RunLeft,
        Self::RunRight,
        Self::Jump,
        Self::Attack,
        Self::Hit,
        Self::Pickup,
        Self::Die,
        Self::Sit,
        Self::Cool,
        Self::Kneel,
        Self::Sad,
        Self::Happy,
        Self::Angry,
        Self::Lie,
        Self::Dance,
        Self::Wave,
        Self::Bow,
        Self::Fly,
        Self::Cast,
        Self::Sweep,
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
            Self::Die => 9,
            Self::Sit => 10,
            Self::Cool => 11,
            Self::Kneel => 12,
            Self::Sad => 13,
            Self::Happy => 14,
            Self::Angry => 15,
            Self::Lie => 16,
            Self::Dance => 17,
            Self::Wave => 18,
            Self::Bow => 19,
            Self::Fly => 20,
            Self::Cast => 21,
            Self::Sweep => 22,
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
            Self::Die => 330,
            Self::Sit => 250,
            Self::Cool => 230,
            Self::Kneel => 210,
            Self::Sad => 170,
            Self::Happy => 150,
            Self::Angry => 160,
            Self::Lie => 270,
            Self::Dance => 1,
            Self::Wave => 190,
            Self::Bow => 200,
            Self::Fly => 510,
            Self::Cast => 903,
            Self::Sweep => 403,
        }
    }

    /// The action for a ChangeAction code, as the original's Action panel
    /// and the server send them.
    pub fn from_catalog_id(id: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.catalog_id() == id && *action != Self::Hit)
    }

    /// Emotes whose last frame is held until the player moves: sitting,
    /// kneeling and lying down.
    pub const fn held(self) -> bool {
        matches!(self, Self::Sit | Self::Kneel | Self::Lie)
    }

    pub const fn emote(self) -> bool {
        matches!(
            self,
            Self::Sit
                | Self::Cool
                | Self::Kneel
                | Self::Sad
                | Self::Happy
                | Self::Angry
                | Self::Lie
                | Self::Dance
                | Self::Wave
                | Self::Bow
        )
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
        assert_eq!(Action::ALL.map(Action::index), std::array::from_fn(|index| index));
        assert_eq!(
            Action::ALL.map(Action::catalog_id),
            [100, 110, 111, 120, 121, 130, 140, 100, 220, 330, 250, 230, 210, 170, 150, 160, 270, 1, 190, 200, 510, 903, 403]
        );
        assert_eq!(Action::from_catalog_id(150), Some(Action::Happy));
        assert_eq!(Action::from_catalog_id(100), Some(Action::Idle));
    }
}
