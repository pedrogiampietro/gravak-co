//! Experience and levels, after the reference server: the `levexp` table
//! (experience per level and the seconds an ExpBall counts for it), combat
//! experience per hit (`CalculateExperienceGain`) and ExpBalls.

use tq_network::Actor;

use crate::entities::Character;
use crate::{ActorState, Error, State};

/// Redux `EXP_RATE`.
pub const EXP_RATE: u64 = 5;
pub const MAX_LEVEL: u8 = 130;
/// An ExpBall is worth this many seconds of the level's `up_lev_time`.
pub const EXP_BALL_SECONDS: f64 = 600.0;
pub const EXP_BALL: u32 = 723_700;
/// Virtue for the team leader per level of a newbie at least this many
/// levels lower (Redux `GainExperience`).
const VIRTUE_LEVEL_GAP: u16 = 20;
const VIRTUE_RANGE: u16 = 25;

/// `levexp`: (experience to leave the level, ExpBall seconds), levels 1-140.
const LEVEL_EXP: [(u64, u32); 140] = [
    (120, 2),
    (180, 2),
    (240, 1),
    (360, 2),
    (600, 2),
    (960, 3),
    (1200, 3),
    (2400, 5),
    (3600, 7),
    (8400, 14),
    (12000, 18),
    (14400, 20),
    (18000, 23),
    (21600, 26),
    (22646, 25),
    (32203, 34),
    (37433, 37),
    (47556, 44),
    (56609, 50),
    (68772, 57),
    (70515, 56),
    (75936, 58),
    (97733, 59),
    (114836, 66),
    (120853, 67),
    (123981, 66),
    (126720, 65),
    (145878, 72),
    (173436, 83),
    (197646, 92),
    (202451, 91),
    (212160, 92),
    (244190, 103),
    (285823, 117),
    (305986, 121),
    (312864, 121),
    (324480, 122),
    (366168, 134),
    (433959, 155),
    (460590, 160),
    (506738, 172),
    (569994, 188),
    (728527, 235),
    (850829, 269),
    (916479, 283),
    (935118, 282),
    (940800, 278),
    (1076593, 312),
    (1272780, 361),
    (1357994, 302),
    (1384861, 302),
    (1478400, 316),
    (1632438, 342),
    (1903104, 392),
    (2066042, 417),
    (2104924, 418),
    (1921085, 374),
    (2417202, 463),
    (2853462, 537),
    (3054574, 566),
    (3111217, 567),
    (3225600, 578),
    (3810962, 672),
    (4437896, 770),
    (4880605, 834),
    (4970962, 837),
    (5107200, 847),
    (5652518, 924),
    (6579162, 1059),
    (6877991, 819),
    (7100700, 833),
    (7157657, 828),
    (9106860, 1040),
    (10596398, 1193),
    (11220549, 831),
    (11409192, 834),
    (11424000, 824),
    (12882952, 918),
    (15172807, 1067),
    (15896990, 1104),
    (16163799, 1109),
    (16800000, 1138),
    (19230280, 1287),
    (22365208, 1479),
    (23819312, 1557),
    (24219528, 1565),
    (24864000, 1588),
    (27200077, 1717),
    (32033165, 2000),
    (33723801, 2082),
    (34291317, 2093),
    (34944000, 1662),
    (39463523, 1877),
    (45878567, 2182),
    (48924236, 2326),
    (49729220, 2365),
    (51072000, 2429),
    (55808379, 2654),
    (64870058, 3085),
    (68391931, 3252),
    (69537026, 3307),
    (76422968, 3634),
    (96950789, 4610),
    (112676755, 5358),
    (120090482, 5710),
    (121798280, 5792),
    (127680000, 6071),
    (137446887, 6536),
    (193715970, 9211),
    (408832150, 9720),
    (454674685, 10810),
    (461125885, 10964),
    (469189885, 11155),
    (477253885, 11347),
    (480479485, 11424),
    (485317885, 11539),
    (493381885, 11730),
    (580580046, 13804),
    (717424987, 17057),
    (282274058, 6711),
    (338728870, 8054),
    (406474644, 9664),
    (487769572, 11597),
    (585323487, 13917),
    (702388184, 16700),
    (842865821, 20040),
    (1011438985, 24048),
    (1073741823, 25527),
    (1073741823, 25527),
    (8589134588, 102106),
    (25767403764, 153159),
    (77302211292, 229739),
    (231906633876, 344608),
    (347859950814, 516912),
    (521789926221, 775368),
    (782684889332, 1163052),
    (1174027333998, 1744578),
    (1761041000997, 2616867),
    (2641561501496, 3925301),
    (0, 0)
];

/// Experience needed to leave `level`; 0 at the top.
pub fn required(level: u8) -> u64 {
    LEVEL_EXP.get(usize::from(level.max(1)) - 1).map_or(0, |(exp, _)| *exp)
}

/// Experience over all levels: every level left behind, and what `level`
/// holds so far (the Powergamers ranking compares these).
pub fn total_experience(level: u8, experience: u64) -> u64 {
    (1..level).map(required).sum::<u64>().saturating_add(experience)
}

fn up_level_time(level: u8) -> u32 {
    LEVEL_EXP.get(usize::from(level.max(1)) - 1).map_or(0, |(_, time)| *time)
}

/// `CalculateExperienceGain` against a monster or a training dummy: the
/// damage dealt (up to the life it had), scaled by the level difference;
/// a tenth on the training ground.
pub fn combat_experience(player_level: u16, target_level: u16, target_life: u32, damage: u32, training: bool) -> u64 {
    let mut experience = f64::from(target_life.min(damage));
    let delta = i32::from(player_level) - i32::from(target_level);
    experience *= match delta {
        3..=5 => 0.7,
        6..=10 => 0.2,
        11..=20 => 0.1,
        21.. => 0.05,
        ..=-16 => 1.8,
        -15..=-9 => 1.5,
        -8..=-6 => 1.3,
        _ => 1.0,
    };
    if training {
        experience /= 10.0;
    }
    experience as u64
}

/// Levels and leftover experience after adding `gained` at `level` with
/// `experience` already inside it.
pub fn advance(mut level: u8, mut experience: u64, gained: u64) -> (u8, u64) {
    experience = experience.saturating_add(gained);
    while level < MAX_LEVEL {
        let need = required(level);
        if need == 0 || experience < need {
            break;
        }
        experience -= need;
        level += 1;
    }
    if level >= MAX_LEVEL {
        experience = experience.min(required(level).saturating_sub(1));
    }
    (level, experience)
}

/// An ExpBall worth `seconds` (Redux `GainExpBall`): whole levels while the
/// time lasts, then a share of the next one.
pub fn exp_ball(mut level: u8, experience: u64, mut seconds: f64) -> (u8, u64) {
    let mut experience = experience;
    while level < MAX_LEVEL {
        let need = required(level);
        let time = f64::from(up_level_time(level));
        if need == 0 || time <= 0.0 {
            break;
        }
        let remaining = time * (need.saturating_sub(experience)) as f64 / need as f64;
        if seconds < remaining {
            experience += (need as f64 / time * seconds) as u64;
            return (level, experience.min(need - 1));
        }
        seconds -= remaining;
        experience = 0;
        level += 1;
    }
    (level, experience)
}

/// After a level up: the team leader earns virtue for helping a newbie.
pub async fn after_level_up(state: &State, character: &Character, new_level: u8) -> Result<(), Error> {
    let members = state.team_manager().members_for(character.id());
    let Some(leader_id) = state.team_manager().leader_for(character.id()) else {
        return Ok(());
    };
    if leader_id == character.id() || !members.contains(&leader_id) {
        return Ok(());
    }
    let Some(leader) = state
        .entities()
        .into_iter()
        .find(|entity| entity.id() == leader_id)
    else {
        return Ok(());
    };
    let Some(leader) = leader.as_character() else {
        return Ok(());
    };
    let here = character.entity().location();
    let there = leader.entity().location();
    let near = leader.entity().map_id() == character.entity().map_id()
        && here.x.abs_diff(there.x).max(here.y.abs_diff(there.y)) < VIRTUE_RANGE;
    if !near || leader.entity().level() < u16::from(new_level) + VIRTUE_LEVEL_GAP {
        return Ok(());
    }
    let virtue = u32::from(new_level) * 20;
    leader.set_virtue(leader.virtue().saturating_add(virtue));
    leader
        .owner()
        .send(crate::packets::MsgTalk::from_system(
            leader.id(),
            crate::packets::TalkChannel::TopLeft,
            format!("Congratulations you have earned {virtue} VirtuePoints by leveling up newbies!"),
        ))
        .await?;
    Ok(())
}

/// Uses one ExpBall's worth of experience on `character` and tells the
/// client; `false` at the top level.
pub async fn use_exp_ball(state: &State, actor: &Actor<ActorState>, character: &Character, seconds: f64) -> Result<bool, Error> {
    let before = character.entity().level();
    if before >= u16::from(MAX_LEVEL) {
        return Ok(false);
    }
    let leveled = character.grant_exp_ball(seconds);
    character.save(state).await?;
    actor.send(crate::packets::MsgUserInfo::from(character)).await?;
    if let Some(level) = leveled {
        actor
            .send(crate::packets::MsgAction::new(
                character.id(),
                character.entity().map_id(),
                0,
                0,
                crate::packets::ActionType::LevelUp,
            ))
            .await?;
        after_level_up(state, character, level).await?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_matches_the_reference() {
        assert_eq!(required(1), 120);
        assert_eq!(required(2), 180);
        assert_eq!(required(140), 0);
    }

    #[test]
    fn experience_carries_over_levels() {
        assert_eq!(advance(1, 0, 119), (1, 119));
        assert_eq!(advance(1, 0, 120), (2, 0));
        assert_eq!(advance(1, 100, 200), (3, 0));
    }

    #[test]
    fn combat_experience_follows_the_level_gap() {
        assert_eq!(combat_experience(50, 50, 1_000, 300, false), 300);
        assert_eq!(combat_experience(60, 50, 1_000, 300, false), 60);
        assert_eq!(combat_experience(50, 70, 1_000, 300, false), 540);
        assert_eq!(combat_experience(50, 50, 100, 300, false), 100);
        assert_eq!(combat_experience(50, 50, 1_000, 300, true), 30);
    }

    #[test]
    fn an_exp_ball_levels_low_characters() {
        let (level, _) = exp_ball(1, 0, EXP_BALL_SECONDS);
        assert!(level > 10);
        let (level, experience) = exp_ball(100, 0, EXP_BALL_SECONDS);
        assert_eq!(level, 100);
        assert!(experience > 0 && experience < required(100));
    }
}
