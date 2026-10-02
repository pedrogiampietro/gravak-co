//! Class trainers of the Promotion Centre, ported from the reference server
//! scripts (`ReduxConquerProject/.../Npcs/Promotion Centre`): Trojan Star,
//! Warrior God, Archer God and Taoist Star. They promote the class through
//! its five tiers and teach the class skills.
//!
//! The script itself is a pure function of the player's state and the chosen
//! option; [`answer`] applies its effects (skills, items, class) afterwards.

use crate::entities::Character;
use crate::packets::{MsgItemInfo, MsgPlayer, MsgSkill, MsgTaskDialog, MsgUserInfo};
use crate::{ActorState, Error, State};
use tq_network::Actor;

const EMERALD: u32 = 1_080_001;
const METEOR: u32 = 1_088_001;
const DRAGONBALL: u32 = 1_088_000;
const MOONBOX: u32 = 721_020;
const POISON_BLADE: u32 = 410_077;
const RAINBOW_GEM: u32 = 700_031;
const UNIQUE_ARMOR: u32 = 130_087;

// Skill ids (`Redux.Enum.SkillID`).
const THUNDER: u32 = 1_000;
const FIRE: u32 = 1_001;
const TORNADO: u32 = 1_002;
const CURE: u32 = 1_005;
const LIGHTNING: u32 = 1_010;
const ACCURACY: u32 = 1_015;
const SHIELD: u32 = 1_020;
const SUPERMAN: u32 = 1_025;
const ROAR: u32 = 1_040;
const REVIVE: u32 = 1_050;
const DASH: u32 = 1_051;
const HEALING_RAIN: u32 = 1_055;
const INVISIBILITY: u32 = 1_075;
const STAR_OF_ACCURACY: u32 = 1_085;
const MAGIC_SHIELD: u32 = 1_090;
const STIGMA: u32 = 1_095;
const PRAY: u32 = 1_100;
const CYCLONE: u32 = 1_110;
const HERCULES: u32 = 1_115;
const FIRE_CIRCLE: u32 = 1_120;
const VOLCANO: u32 = 1_125;
const FIRE_RING: u32 = 1_150;
const NECTAR: u32 = 1_170;
const ADVANCED_CURE: u32 = 1_175;
const FIRE_METEOR: u32 = 1_180;
const SPIRIT_HEALING: u32 = 1_190;
const MEDITATION: u32 = 1_195;
const ROBOT: u32 = 1_270;
const SPEED_LIGHTNING: u32 = 5_001;
const RAPID_FIRE: u32 = 8_000;
const SCATTER: u32 = 8_001;
const FLY: u32 = 8_002;
const ADVANCED_FLY: u32 = 8_003;
const ARROW_RAIN: u32 = 8_030;
const INTENSIFY: u32 = 9_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Trojan,
    Warrior,
    Archer,
    Taoist,
}

impl Family {
    pub fn of_class(class: u8) -> Option<Self> {
        Some(match class {
            10..=15 => Self::Trojan,
            20..=25 => Self::Warrior,
            40..=45 => Self::Archer,
            100 | 101 | 130..=135 | 140..=145 => Self::Taoist,
            _ => return None,
        })
    }

    /// The trainers by their ids in our NPC table (the Promotion Centre ids
    /// match the reference; the extra Taoist Stars stand in other maps).
    pub fn of_npc(npc_id: u32) -> Option<Self> {
        Some(match npc_id {
            10_022 | 3_117 => Self::Trojan,
            10_001 => Self::Warrior,
            400 => Self::Archer,
            10_000 | 30 | 10_055 | 3_123 => Self::Taoist,
            _ => return None,
        })
    }

    fn face(self) -> u16 {
        match self {
            Self::Trojan => 155,
            Self::Warrior => 8,
            Self::Archer => 10,
            Self::Taoist => 6,
        }
    }
}

/// What the player brings to the trainer.
#[derive(Debug, Clone, Default)]
pub struct Student {
    pub class: u8,
    pub level: u16,
    pub strength: u16,
    pub agility: u16,
    pub vitality: u16,
    pub spirit: u16,
    pub rebirths: u8,
    pub items: Vec<u32>,
    pub skills: Vec<(u32, u16)>,
}

impl Student {
    fn tier(&self) -> u8 {
        self.class % 10
    }

    fn has(&self, item_type: u32) -> bool {
        self.items.contains(&item_type)
    }

    fn knows_level(&self, skill_id: u32, level: u16) -> bool {
        self.skills
            .iter()
            .any(|(skill, known)| *skill == skill_id && *known >= level)
    }

    fn is_fire(&self) -> bool {
        (140..=145).contains(&self.class)
    }

    fn is_water(&self) -> bool {
        (130..=135).contains(&self.class)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Learn(u32),
    Take(u32),
    Give { item_type: u32, open_socket: bool },
    SetClass(u8),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reply {
    pub lines: Vec<String>,
    pub options: Vec<(u8, String)>,
    pub effects: Vec<Effect>,
}

impl Reply {
    fn text(mut self, line: &str) -> Self {
        self.lines.push(line.to_owned());
        self
    }

    fn option(mut self, id: u8, label: &str) -> Self {
        self.options.push((id, label.to_owned()));
        self
    }

    fn effect(mut self, effect: Effect) -> Self {
        self.effects.push(effect);
        self
    }
}

fn reply() -> Reply {
    Reply::default()
}

/// Requirements of the promotion out of tier `tier` (0..=4):
/// level, strength, agility, vitality, spirit and the item consumed.
fn requirement(family: Family, tier: u8) -> Option<(u16, u16, u16, u16, u16, Option<u32>)> {
    use Family::*;
    Some(match (family, tier) {
        (_, 0) => (15, 0, 0, 0, 0, None),
        (Trojan, 1) => (40, 60, 25, 25, 0, None),
        (Trojan, 2) => (70, 110, 42, 45, 0, Some(EMERALD)),
        (Trojan, 3) => (100, 110, 42, 45, 0, Some(METEOR)),
        (Trojan, 4) => (110, 170, 65, 100, 0, Some(MOONBOX)),
        (Warrior, 1) => (40, 80, 25, 20, 0, None),
        (Warrior, 2) => (70, 140, 45, 32, 0, Some(EMERALD)),
        (Warrior, 3) => (100, 205, 60, 42, 0, Some(METEOR)),
        (Warrior, 4) => (110, 225, 65, 47, 0, Some(MOONBOX)),
        (Archer, 1) => (40, 25, 90, 12, 0, None),
        (Archer, 2) => (70, 45, 150, 2, 0, Some(EMERALD)),
        (Archer, 3) => (100, 60, 215, 32, 0, Some(METEOR)),
        (Archer, 4) => (110, 68, 235, 34, 0, Some(MOONBOX)),
        (Taoist, 1) => (40, 0, 25, 22, 80, None),
        (Taoist, 2) => (70, 0, 45, 32, 140, Some(EMERALD)),
        (Taoist, 3) => (100, 0, 60, 42, 205, Some(METEOR)),
        (Taoist, 4) => (110, 0, 65, 47, 225, Some(MOONBOX)),
        _ => return None,
    })
}

fn meets(student: &Student, family: Family, tier: u8) -> bool {
    requirement(family, tier).is_some_and(|(level, strength, agility, vitality, spirit, item)| {
        student.level >= level
            && student.strength >= strength
            && student.agility >= agility
            && student.vitality >= vitality
            && student.spirit >= spirit
            && item.is_none_or(|item| student.has(item))
    })
}

fn introduction(family: Family, student: &Student) -> Reply {
    match family {
        Family::Trojan => reply()
            .text("Trojans can wield dual weapons, and fearlessly take on anyone in melee")
            .text(" combat! Whatever the cost, always remember that courage is the foundation")
            .text(" of victory! So, what can I do for you?")
            .option(1, "I would like to get promoted.")
            .option(3, "I wish to learn skills.")
            .option(255, "Nothing."),
        Family::Warrior => reply()
            .text("To be a good Warrior, you must know your weapon, your armor be a 2nd")
            .text(" skin, and use your shild to ward off your enemy! Remember, fortune favors")
            .text(" the brave, and you must keep your head, even in the darkest hours. So, what")
            .text(" can I do for you?")
            .option(1, "I want to get promoted.")
            .option(3, "Learn class skills.")
            .option(255, "Okay. I see."),
        Family::Archer => reply()
            .text("Mercilessly fight your way to the top, my friend, because Archers are destined ")
            .text("for greatness! Your swift mind will give you the ability to attack from afar, at a ")
            .text("pace unmatched by any of your peers. So, what can I do for you?")
            .option(1, "I want to get promoted.")
            .option(3, "Learn class skills.")
            .option(255, "Okay. I see."),
        Family::Taoist => {
            let reply = reply()
                .text("Most of the Taoists are little concerned with anything outside the pursuit of ")
                .text("advanced spiritual powers. You are gifted in harnessing your inner power, ")
                .text("but remember, the roots of success lie in thoroughness and attention to detail.")
                .option(1, "I want to get promoted.")
                .option(5, "Learn basic skills.");
            let reply = if student.tier() >= 2 {
                reply.option(6, "Learn advanced skills.")
            } else {
                reply
            };
            reply.option(255, "Okay. I see.")
        },
    }
}

/// Rank names used in the promotion offers, by the tier being reached.
fn rank(family: Family, student: &Student, tier: u8) -> &'static str {
    match (family, tier) {
        (Family::Trojan, 2) => "a Veteran Trojan",
        (Family::Trojan, 3) => "Tiger Trojan",
        (Family::Trojan, 4) => "Dragon Trojan",
        (Family::Trojan, _) => "Trojan Master",
        (Family::Warrior, 2) => "a Brass Warrior",
        (Family::Warrior, 3) => "Silver Warrior",
        (Family::Warrior, 4) => "Gold Warrior",
        (Family::Warrior, _) => "Warrior Master",
        (Family::Archer, 2) => "a Eagle Archer",
        (Family::Archer, 3) => "Tiger Archer",
        (Family::Archer, 4) => "Dragon Archer",
        (Family::Archer, _) => "Archer Master",
        (Family::Taoist, 3) if student.is_fire() => "a Fire Wizard",
        (Family::Taoist, 3) => "a Water Wizard",
        (Family::Taoist, 4) if student.is_fire() => "a Fire Master",
        (Family::Taoist, 4) => "a Water Master",
        (Family::Taoist, _) if student.is_fire() => "a Fire Saint",
        (Family::Taoist, _) => "a Water Saint",
    }
}

fn class_name(family: Family) -> &'static str {
    match family {
        Family::Trojan => "Trojan",
        Family::Warrior => "Warrior",
        Family::Archer => "Archer",
        Family::Taoist => "Taoist",
    }
}

/// Option 1: what the next promotion needs.
fn promotion_offer(family: Family, student: &Student) -> Reply {
    let tier = student.tier();
    let level = requirement(family, tier).map_or(0, |requirement| requirement.0);
    let ready = |reply: Reply| reply.option(2, "I'm ready!").option(255, "I'm not sure...");
    match tier {
        0 => reply()
            .text(&format!(
                "The single requirement of becoming {} {} is a minimum level of 15.",
                if family == Family::Archer { "an" } else { "a" },
                class_name(family)
            ))
            .text(" Would you like to proceed?")
            .option(2, "Yes, please.")
            .option(255, "No."),
        1 if family == Family::Taoist => reply()
            .text("A decision stands before you! Become an all powerful fire taoist or a merciful water taoist?")
            .option(3, "Water Taoist")
            .option(4, "Fire Taoist")
            .option(255, "No."),
        1 => reply()
            .text(&format!(
                "You may become {} at level 40. Do you want to continue?",
                rank(family, student, 2)
            ))
            .option(2, "Yes, I do.")
            .option(255, "No."),
        2 | 3 | 4 => {
            let (item_line, ask) = match tier {
                2 => (
                    " I will require one Emerald, I hear they are dropped by",
                    "  monsters in the desert. Are you ready for promotion?",
                ),
                3 => (" I will require one Meteor. Are you ready for promotion?", ""),
                _ => (" I will require one Moon Box. Are you ready for promotion?", ""),
            };
            let verb = if family == Family::Taoist {
                "promote to become"
            } else {
                "promote to"
            };
            let rank = rank(family, student, tier + 1);
            let rank = rank.strip_prefix("a ").filter(|_| family != Family::Taoist).unwrap_or(rank);
            let mut reply = reply().text(&format!("You may {verb} {rank} at level {level}."));
            reply = reply.text(item_line);
            if !ask.is_empty() {
                reply = reply.text(ask);
            }
            ready(reply)
        },
        _ => reply()
            .text(&format!(
                "You are a true {}. I cannot promote you anymore!",
                class_name(family)
            ))
            .option(255, "I see."),
    }
}

/// Options 2 (and 3/4 for the Taoist path choice): the promotion itself.
fn promote(family: Family, student: &Student, fire: Option<bool>) -> Reply {
    let tier = student.tier();
    let refuse = |line: &str| reply().text(line).option(255, "I'll work harder.");
    if !meets(student, family, tier) {
        return match tier {
            0 => refuse("You are not level 15, I will not promote you yet."),
            1 => refuse("You must be level 40, I will not promote you yet."),
            2 => refuse("You must be level 70 and bring me an Emerald in order to be promoted!"),
            3 => refuse("You must be level 100 and have a Meteor in order to be promoted!"),
            4 => refuse("You must be level 110 and have a Moon Box in order to be promoted!"),
            _ => reply(),
        };
    }
    let next_class = match (family, tier, fire) {
        (Family::Taoist, 1, Some(true)) => 142,
        (Family::Taoist, 1, Some(false)) => 132,
        (Family::Taoist, 1, None) => return promotion_offer(family, student),
        _ => student.class + 1,
    };
    let mut reply = match tier {
        0 if family == Family::Taoist => reply()
            .text("You have been promoted! I have taught you an XP skill as a reward!")
            .effect(Effect::Learn(THUNDER)),
        0 => {
            let skills: &[u32] = match family {
                Family::Trojan => &[ACCURACY, CYCLONE],
                Family::Warrior => &[ACCURACY, SHIELD, SUPERMAN, ROAR],
                _ => &[FLY],
            };
            skills.iter().fold(
                reply().text("You have been promoted! I have taught you XP skills as a reward!"),
                |reply, skill| reply.effect(Effect::Learn(*skill)),
            )
        },
        _ => reply().text("You have been promoted!"),
    };
    if let Some(item) = requirement(family, tier).and_then(|requirement| requirement.5) {
        reply = reply.effect(Effect::Take(item));
    }
    let give = |item_type| Effect::Give {
        item_type,
        open_socket: false,
    };
    reply = match tier {
        1 if student.rebirths == 1 => reply
            .text("I've given you a special item to help commemorate the occasion!")
            .effect(give(POISON_BLADE)),
        2 => reply.effect(give(METEOR)),
        3 if student.rebirths == 0 => reply.effect(give(RAINBOW_GEM)),
        3 => reply.effect(Effect::Give {
            item_type: UNIQUE_ARMOR,
            open_socket: true,
        }),
        4 => reply.effect(give(DRAGONBALL)),
        _ => reply,
    };
    reply.effect(Effect::SetClass(next_class)).option(255, "Thank you!")
}

/// Learns `skills` when `level` is reached, as the "learn" options do.
fn teach(student: &Student, level: u16, skills: &[u32], learned: &str, too_low: &str) -> Reply {
    let reply = if student.level >= level {
        skills
            .iter()
            .fold(reply().text(learned), |reply, skill| reply.effect(Effect::Learn(*skill)))
    } else {
        reply().text(too_low)
    };
    reply.option(255, "Thanks")
}

fn skill_menu(family: Family, student: &Student, option: u8) -> Option<Reply> {
    let one = |level: u16, skill: u32, name: &str| {
        teach(
            student,
            level,
            &[skill],
            &format!("You have learned {name} successfully"),
            &format!("You must be at least level {level} to learn this skill!"),
        )
    };
    Some(match (family, option) {
        (Family::Trojan, 3) => reply()
            .text("What skill would you like to learn?")
            .option(4, "XP Skills")
            .option(5, "Hercules")
            .option(6, "Spirit Healing")
            .option(7, "Robot")
            .option(255, "Nothing."),
        (Family::Trojan, 4) => teach(
            student,
            15,
            &[CYCLONE, ACCURACY],
            "You have learned Cyclone and Accuracy successfully",
            "You must be at least level 15 to learn these skills!",
        ),
        (Family::Trojan, 5) => one(40, HERCULES, "Hercules"),
        (Family::Trojan, 6) => one(40, SPIRIT_HEALING, "Spirit Healing"),
        (Family::Trojan, 7) => one(40, ROBOT, "Robot"),
        (Family::Warrior, 3) => reply()
            .text("What skill would you like to learn?")
            .option(4, "XP Skills")
            .option(5, "Dash")
            .option(255, "Nothing."),
        (Family::Warrior, 4) => teach(
            student,
            15,
            &[ACCURACY, SHIELD, SUPERMAN, ROAR],
            "You have learned your XP skills successfully",
            "You must be at least level 15 to learn these skills!",
        ),
        (Family::Warrior, 5) => one(63, DASH, "Dash"),
        (Family::Archer, 3) => reply()
            .text("The Archer's Skills are numerous, varied and powerful. Which would you like to learn?")
            .option(4, "Fly")
            .option(5, "Scatter")
            .option(6, "Rapid Fire")
            .option(7, "Intensify")
            .option(8, "Other Skills")
            .option(255, "Nothing."),
        (Family::Archer, 8) => reply()
            .text("The Archer's Skills are numerous, varied and powerful. Which would you like to learn?")
            .option(9, "Advanced Fly")
            .option(10, "Arrow Rain")
            .option(3, "Previous Skills")
            .option(255, "Nothing."),
        (Family::Archer, 4) => one(15, FLY, "Fly"),
        (Family::Archer, 5) => one(23, SCATTER, "Scatter"),
        (Family::Archer, 6) => one(46, RAPID_FIRE, "Rapid Fire"),
        (Family::Archer, 7) => one(71, INTENSIFY, "Intensify"),
        (Family::Archer, 9) => one(70, ADVANCED_FLY, "Advanced Fly"),
        (Family::Archer, 10) => one(70, ARROW_RAIN, "Arrow Rain"),
        (Family::Taoist, 5) => reply()
            .text("What skill would you like to learn?")
            .option(10, "Basic Spells")
            .option(11, "Xp Skills")
            .option(12, "Fire")
            .option(13, "Meditation")
            .option(255, "Nothing."),
        (Family::Taoist, 6) if student.is_fire() => reply()
            .text("What skill would you like to learn?")
            .option(14, "Fire Ring")
            .option(15, "Fire Meteor")
            .option(16, "Fire Circle")
            .option(17, "Tornado")
            .option(255, "Nothing."),
        (Family::Taoist, 6) if student.is_water() => reply()
            .text("What skill would you like to learn?")
            .option(18, "Healing Rain")
            .option(19, "Star of Accuracy")
            .option(20, "Magic Shield")
            .option(21, "Stigma")
            .option(30, "More skills")
            .option(255, "Nothing."),
        (Family::Taoist, 30) if student.is_water() => reply()
            .text("What skill would you like to learn?")
            .option(22, "Invisibility")
            .option(23, "Pray")
            .option(24, "Advanced Cure")
            .option(25, "Nectar")
            .option(6, "Previous skills")
            .option(255, "Nothing."),
        (Family::Taoist, 6 | 30) => reply()
            .text("What! You don't belong here. I can only teach these skills to advanced students.")
            .option(255, "Nevermind"),
        (Family::Taoist, 10) => reply()
            .text("Learned all basic spells.")
            .effect(Effect::Learn(CURE))
            .effect(Effect::Learn(THUNDER))
            .option(255, "Thanks"),
        (Family::Taoist, 11) => {
            if student.level < 15 {
                reply()
                    .text("You must be at least level 15 to learn XP skills")
                    .option(255, "Thanks")
            } else {
                let mut reply = reply()
                    .text("Learned all available XP skills")
                    .effect(Effect::Learn(LIGHTNING));
                if student.level >= 40 {
                    reply = reply.effect(Effect::Learn(VOLCANO));
                    if student.is_water() {
                        reply = reply.effect(Effect::Learn(REVIVE));
                    }
                }
                if student.level >= 70 {
                    reply = reply.effect(Effect::Learn(SPEED_LIGHTNING));
                }
                reply.option(255, "Thanks")
            }
        },
        (Family::Taoist, 12) => {
            if student.level >= 40 && student.knows_level(THUNDER, 4) {
                reply()
                    .text("You have learned Fire successfully")
                    .effect(Effect::Learn(FIRE))
            } else {
                reply().text("You must be at least level 40 with level 4 Thunder to learn this skill")
            }
            .option(255, "Thanks")
        },
        (Family::Taoist, 13) => one(44, MEDITATION, "Meditation"),
        (Family::Taoist, 14..=17) if !student.is_fire() => return None,
        (Family::Taoist, 14) => one(55, FIRE_RING, "Fire Ring"),
        (Family::Taoist, 15) => one(52, FIRE_METEOR, "Fire Meteor"),
        (Family::Taoist, 16) => one(65, FIRE_CIRCLE, "Fire Circle"),
        (Family::Taoist, 17) => {
            if student.level >= 90 && student.knows_level(FIRE, 3) {
                reply()
                    .text("You have learned Tornado successfully")
                    .effect(Effect::Learn(TORNADO))
            } else {
                reply().text("You must be at least level 90 with level 3 Fire to learn this skill")
            }
            .option(255, "Thanks")
        },
        (Family::Taoist, 18..=25) if !student.is_water() => return None,
        (Family::Taoist, 18) => one(40, HEALING_RAIN, "Healing Rain"),
        (Family::Taoist, 19) => one(45, STAR_OF_ACCURACY, "Star of Accuracy"),
        (Family::Taoist, 20) => one(50, MAGIC_SHIELD, "Magic Shield"),
        (Family::Taoist, 21) => one(55, STIGMA, "Stigma"),
        (Family::Taoist, 22) => one(60, INVISIBILITY, "Invisibility"),
        (Family::Taoist, 23) => one(70, PRAY, "Pray"),
        (Family::Taoist, 24) => one(81, ADVANCED_CURE, "Advanced Cure"),
        (Family::Taoist, 25) => one(94, NECTAR, "Nectar"),
        _ => return None,
    })
}

/// The trainer script: `None` when the option does nothing (for example 255).
pub fn script(family: Family, student: &Student, option: u8) -> Option<Reply> {
    if Family::of_class(student.class) != Some(family) {
        let line = match family {
            Family::Taoist => "The taoists` secrets are not to be trained to outsiders. Train elsewhere, master conqueror.",
            Family::Trojan => "Trojans do not share their secrets of battle with others. I shall not teach you.",
            Family::Warrior => "Warriors do not share their secrets of battle with others. I shall not teach you.",
            Family::Archer => "Archers do not share their secrets of battle with others. I shall not teach you.",
        };
        return (option == 0).then(|| reply().text(line).option(255, "I see."));
    }
    match option {
        0 => Some(introduction(family, student)),
        1 => Some(promotion_offer(family, student)),
        2 => Some(promote(family, student, None)),
        3 | 4 if family == Family::Taoist && student.tier() == 1 => Some(promote(family, student, Some(option == 4))),
        _ => skill_menu(family, student, option),
    }
}

/// Opening dialog for the NPC activation: text, options and face.
pub fn opening(npc_id: u32, character: &Character) -> Option<(String, Vec<(u8, String)>, u16)> {
    let family = Family::of_npc(npc_id)?;
    let reply = script(family, &student(character), 0)?;
    Some((reply.lines.concat(), reply.options, family.face()))
}

fn student(character: &Character) -> Student {
    Student {
        class: character.current_class(),
        level: character.entity().level(),
        strength: character.strength(),
        agility: character.agility(),
        vitality: character.vitality(),
        spirit: character.spirit(),
        rebirths: character.rebirths(),
        items: character.inventory_items().iter().map(|item| item.item_type).collect(),
        skills: character
            .skills()
            .iter()
            .map(|skill| (skill.skill_id, skill.level))
            .collect(),
    }
}

/// Handles an answer to a trainer. Returns false for other NPCs.
pub async fn answer(
    state: &State,
    actor: &Actor<ActorState>,
    character: &Character,
    npc_id: u32,
    option: u8,
) -> Result<bool, Error> {
    let Some(family) = Family::of_npc(npc_id) else {
        return Ok(false);
    };
    let Some(reply) = script(family, &student(character), option) else {
        return Ok(true);
    };
    for effect in &reply.effects {
        match *effect {
            Effect::Learn(skill_id) => {
                if let Some(progress) = character.learn_new_skill(state.pool(), skill_id).await? {
                    actor.send(MsgSkill::from(progress)).await?;
                }
            },
            Effect::Take(item_type) => {
                if let Some(item) = character.take_item(state.pool(), item_type).await? {
                    actor.send(MsgItemInfo::remove_inventory_item(item)).await?;
                }
            },
            Effect::Give {
                item_type,
                open_socket,
            } => {
                if let Some(item) = character
                    .grant_item(state.pool(), state.shop_catalog(), item_type, open_socket)
                    .await?
                {
                    actor.send(MsgItemInfo::inventory_item(item)).await?;
                }
            },
            Effect::SetClass(class) => {
                character.set_class(state.pool(), class).await?;
                actor.send(MsgUserInfo::from(character)).await?;
                actor.screen().send_message(MsgPlayer::from(character)).await?;
                crate::audit::economic("promotion", character.id(), npc_id, u32::from(class), 0);
            },
        }
    }
    let mut builder = MsgTaskDialog::builder().text(reply.lines.concat());
    for (id, label) in reply.options {
        builder = builder.with_option(id, label);
    }
    actor
        .send_all(builder.and().with_avatar(family.face()).task_id(npc_id).build())
        .await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn student(class: u8, level: u16) -> Student {
        Student {
            class,
            level,
            ..Student::default()
        }
    }

    #[test]
    fn trainers_only_teach_their_own_class() {
        let reply = script(Family::Warrior, &student(10, 50), 0).expect("refusal");
        assert!(reply.lines[0].starts_with("Warriors do not share"));
        assert_eq!(script(Family::Warrior, &student(10, 50), 1), None);
    }

    #[test]
    fn first_promotion_needs_level_15_and_teaches_xp_skills() {
        let low = script(Family::Trojan, &student(10, 14), 2).expect("refusal");
        assert_eq!(low.lines, ["You are not level 15, I will not promote you yet."]);
        assert!(low.effects.is_empty());
        let ok = script(Family::Trojan, &student(10, 15), 2).expect("promotion");
        assert_eq!(
            ok.effects,
            [
                Effect::Learn(ACCURACY),
                Effect::Learn(CYCLONE),
                Effect::SetClass(11)
            ]
        );
    }

    #[test]
    fn later_promotions_consume_and_reward_items() {
        let mut archer = student(42, 70);
        archer.strength = 45;
        archer.agility = 150;
        archer.vitality = 2;
        assert!(script(Family::Archer, &archer, 2).expect("refusal").effects.is_empty());
        archer.items.push(EMERALD);
        assert_eq!(
            script(Family::Archer, &archer, 2).expect("promotion").effects,
            [
                Effect::Take(EMERALD),
                Effect::Give {
                    item_type: METEOR,
                    open_socket: false
                },
                Effect::SetClass(43)
            ]
        );
    }

    #[test]
    fn taoists_choose_fire_or_water_at_level_40() {
        let mut taoist = student(101, 40);
        taoist.agility = 25;
        taoist.vitality = 22;
        taoist.spirit = 80;
        let offer = script(Family::Taoist, &taoist, 1).expect("choice");
        assert_eq!(offer.options[0], (3, "Water Taoist".to_owned()));
        let fire = script(Family::Taoist, &taoist, 4).expect("fire");
        assert_eq!(fire.effects.last(), Some(&Effect::SetClass(142)));
        let water = script(Family::Taoist, &taoist, 3).expect("water");
        assert_eq!(water.effects.last(), Some(&Effect::SetClass(132)));
        // Advanced skills only after the choice.
        assert!(!script(Family::Taoist, &taoist, 0)
            .expect("intro")
            .options
            .iter()
            .any(|(id, _)| *id == 6));
    }

    #[test]
    fn skills_follow_the_level_requirements() {
        let reply = script(Family::Warrior, &student(21, 62), 5).expect("dash");
        assert_eq!(reply.lines, ["You must be at least level 63 to learn this skill!"]);
        let reply = script(Family::Warrior, &student(21, 63), 5).expect("dash");
        assert_eq!(reply.effects, [Effect::Learn(DASH)]);
        let mut fire = student(142, 90);
        fire.skills.push((FIRE, 3));
        assert_eq!(
            script(Family::Taoist, &fire, 17).expect("tornado").effects,
            [Effect::Learn(TORNADO)]
        );
        assert_eq!(script(Family::Taoist, &student(132, 90), 17), None);
    }

    #[test]
    fn offers_name_the_next_rank() {
        let offer = script(Family::Trojan, &student(12, 60), 1).expect("offer");
        assert_eq!(offer.lines[0], "You may promote to Tiger Trojan at level 70.");
        let offer = script(Family::Warrior, &student(21, 30), 1).expect("offer");
        assert_eq!(offer.lines[0], "You may become a Brass Warrior at level 40. Do you want to continue?");
        let offer = script(Family::Taoist, &student(144, 100), 1).expect("offer");
        assert_eq!(offer.lines[0], "You may promote to become a Fire Saint at level 110.");
    }
}
