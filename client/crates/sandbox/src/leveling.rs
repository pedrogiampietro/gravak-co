//! Levels as the server counts them (Redux `levelexp`): the experience
//! each level needs and the seconds an ExpBall counts for it, so the
//! client can show what an ExpBall would give before it is used.

/// The ExpBall, worth ten minutes of the current level.
pub const EXP_BALL: u32 = 723_700;
pub const EXP_BALL_SECONDS: f64 = 600.0;
pub const MAX_LEVEL: u8 = 130;

/// Experience and the seconds it takes, per level from 1.
pub const LEVEL_EXP: [(u64, u32); 140] = [
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

/// Experience `level` needs to go up.
/// How far into `level` the hero is: "Level 45: 24.37% (12,345 / 50,660)".
pub fn progress_line(level: u8, experience: u64) -> String {
    let need = required(level);
    if need == 0 {
        return format!("Level {level}: the top level");
    }
    let percent = experience as f64 * 100.0 / need as f64;
    let group = |value: u64| {
        let digits = value.to_string();
        let mut out = String::new();
        for (index, digit) in digits.chars().enumerate() {
            if index > 0 && (digits.len() - index) % 3 == 0 {
                out.push(',');
            }
            out.push(digit);
        }
        out
    };
    format!("Level {level}: {percent:.2}% ({} / {})", group(experience), group(need))
}

pub fn required(level: u8) -> u64 {
    LEVEL_EXP.get(usize::from(level.max(1)) - 1).map_or(0, |(experience, _)| *experience)
}

/// What an ExpBall worth `seconds` brings (the server's `exp_ball`): whole
/// levels while the time lasts, then a share of the next.
pub fn exp_ball(mut level: u8, mut experience: u64, mut seconds: f64) -> (u8, u64) {
    while level < MAX_LEVEL {
        let Some(&(need, time)) = LEVEL_EXP.get(usize::from(level.max(1)) - 1) else {
            break;
        };
        if need == 0 || time == 0 {
            break;
        }
        let time = f64::from(time);
        let remaining = time * need.saturating_sub(experience) as f64 / need as f64;
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

#[cfg(test)]
mod tests {
    #[test]
    fn the_bar_tells_how_far_into_the_level() {
        let need = super::required(45);
        assert_eq!(super::progress_line(45, need / 4), format!("Level 45: 25.00% ({} / {})", group(need / 4), group(need)));
        assert_eq!(super::progress_line(1, 0), "Level 1: 0.00% (0 / 120)");
        assert_eq!(super::progress_line(2, 45), "Level 2: 25.00% (45 / 180)");
    }

    fn group(value: u64) -> String {
        let digits = value.to_string();
        let mut out = String::new();
        for (index, digit) in digits.chars().enumerate() {
            if index > 0 && (digits.len() - index) % 3 == 0 {
                out.push(',');
            }
            out.push(digit);
        }
        out
    }

    #[test]
    fn an_exp_ball_fills_ten_minutes_of_the_level() {
        let (level, _) = super::exp_ball(1, 0, super::EXP_BALL_SECONDS);
        assert!(level > 1);
        let (level, experience) = super::exp_ball(100, 0, 1.0);
        assert_eq!(level, 100);
        assert!(experience > 0);
    }
}
