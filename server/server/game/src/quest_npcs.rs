//! The Labyrinth, the Moon Box quest and Milly's love letter, after the
//! reference server scripts (`Npcs/Lab/*.cs`, `Npcs/Moon Box Quest/*.cs`,
//! Milly, Minner and Joe).

use tq_network::Actor;

use crate::entities::Character;
use crate::packets::{MsgItemInfo, MsgTaskDialog, MsgUserInfo};
use crate::{ActorState, Error, State};

pub const SIMON_ID: u32 = 1_152;
pub const FORTUNETELLER_ID: u32 = 600_076;
pub const MAGGIE_ID: u32 = 6_100;
/// GuardianGod (6110..) and Ghost (6120..) of the seven tactics, in the
/// order Peace, Chaos, Deserted, Prosperous, Disturbed, Calmed, Death.
const GUARDIAN_GOD_ID: u32 = 6_110;
const GHOST_ID: u32 = 6_120;
const LONELY_GHOST_ID: u32 = 7_660;
const LONELY_GHOSTS: u32 = 6;

/// Old Explorers: (npc, token, destination map, x, y, token name).
const EXPLORERS: [(u32, u32, u32, u16, u16, &str); 3] = [
    (1_339, 721_537, 1_352, 104, 108, "SkyToken"),
    (1_340, 721_538, 1_353, 321, 123, "EarthToken"),
    (1_341, 721_539, 1_354, 321, 123, "SoulToken"),
];
/// The Old Explorer on the last floor.
const LAST_EXPLORER_ID: u32 = 1_342;

const LABYRINTH: (u32, u16, u16) = (1_351, 100, 100);
const LABYRINTH_VIRTUE: u32 = 2_000;
const TWIN_CITY: (u32, u16, u16) = (1_002, 393, 240);
const MOON_BOX_ENTRY: (u32, u16, u16) = (1_042, 35, 48);
const DEATH_TACTIC: u32 = 1_049;
const LIFE_TACTIC: u32 = 1_050;
const TACTIC_ENTRY: (u16, u16) = (211, 156);
/// Leaving the Life Tactic.
const LIFE_TACTIC_EXIT: (u32, u16, u16) = (1_002, 417, 516);

/// The six tactic tokens (Peace .. Calmed).
const FIRST_TOKEN: u32 = 721_010;
const SOUL_JADE: u32 = 721_072;
const MOONBOX: u32 = 721_020;
const METEOR_SCROLL: u32 = 720_027;

/// Simon's diamond trades: (option, diamond, count, reward item, silver).
const DIAMONDS: [(u8, u32, usize, u32, u32); 4] = [
    (6, 721_533, 15, METEOR_SCROLL, 0),
    (7, 721_534, 12, METEOR_SCROLL, 250_000),
    (8, 721_535, 12, METEOR_SCROLL, 500_000),
    (9, 721_536, 10, 721_541, 1_000_000),
];

/// The love letter: Milly (Phoenix Castle) writes to her sister Minner (Bird
/// Island), whose lover Joe (Desert City) left; a MeteorTear rewards it.
pub const MILLY_ID: u32 = 600_000;
pub const MINNER_ID: u32 = 600_001;
pub const JOE_ID: u32 = 600_002;
const LETTER: u32 = 721_000;
const LOVE_BAG: u32 = 721_001;
const SAD_METEOR: u32 = 721_002;
const AMRITA: u32 = 1_000_030;
const METEOR: u32 = 1_088_001;
const METEOR_TEAR: u32 = 1_088_002;

const TACTICS: [&str; 7] = ["Peace", "Chaos", "Deserted", "Prosperous", "Disturbed", "Calmed", "Death"];

type Options = Vec<(u8, String)>;

fn options(list: &[(u8, &str)]) -> Options {
    list.iter().map(|(id, label)| (*id, (*label).to_owned())).collect()
}

fn tactic(npc_id: u32, base: u32) -> Option<usize> {
    npc_id
        .checked_sub(base)
        .map(|index| index as usize)
        .filter(|index| *index < TACTICS.len())
}

fn has_all_tokens(character: &Character) -> bool {
    (0..6).all(|i| character.count_items(FIRST_TOKEN + i) > 0)
}

fn face(npc_id: u32) -> u16 {
    match npc_id {
        SIMON_ID => 1,
        FORTUNETELLER_ID | MAGGIE_ID => 20,
        _ => 32,
    }
}

pub fn opening(npc_id: u32, character: &Character) -> Option<(String, Options, u16)> {
    let (text, options) = if npc_id == SIMON_ID {
        (
            "Where are you heading for? I can teleport you into an dark place full of caves and strong monsters for  VirtuePoints .Make sure to gather all diamonds dropped by monsters as I will reward you nicely for them !".to_owned(),
            options(&[(1, "Send me there!"), (5, "I have some diamonds"), (255, "Just passing by.")]),
        )
    } else if let Some((_, _, _, _, _, token)) = EXPLORERS.iter().find(|explorer| explorer.0 == npc_id) {
        (
            format!("Do you wish to enter the next level of labyrinth? Give me an {token}"),
            options(&[(1, "Yes, send me to the quest map."), (255, "No Thanks.")]),
        )
    } else if npc_id == LAST_EXPLORER_ID {
        (
            "This is the deepest level of the labyrinth. Do you wish to go back to Twin City?".to_owned(),
            options(&[(1, "Yes, take me back."), (255, "No Thanks.")]),
        )
    } else if npc_id == MILLY_ID {
        if character.count_items(LETTER) > 0 {
            (
                "My sister is in Bird Island. She fell in love with Joe there. Since Joe left her with no news, she stays there to wait for him.".to_owned(),
                options(&[(255, "I see.")]),
            )
        } else {
            (
                "I cannot get near you now. You have got me going crazy. Wherever you go, whatever you do, I will be right here waiting for you.".to_owned(),
                options(&[(1, "Why are you so sad?"), (255, "I got to go.")]),
            )
        }
    } else if npc_id == MINNER_ID {
        if character.count_items(SAD_METEOR) > 0 {
            (
                "I cannot help crying. Where did you get this sad meteor? What happened?".to_owned(),
                options(&[(6, "It is from Joe."), (255, "Sorry, I cannot tell you.")]),
            )
        } else {
            (
                "Should auld acquaintance be forgotten and never brought to mind? Should auld acquaintance be forgotten and auld lang syne?".to_owned(),
                options(&[(1, "Why are you so sad?"), (255, "Nice song.")]),
            )
        }
    } else if npc_id == JOE_ID {
        if character.count_items(SAD_METEOR) > 0 {
            (
                "Thanks a lot. Please kindly give this meteor to Minner. I hope she can understand me and wish her happy all her life.".to_owned(),
                options(&[(255, "I hope so.")]),
            )
        } else {
            (
                "It is so hot. It would be very nice If I have a bottle of wine. Can you give me some wine?".to_owned(),
                options(&[(1, "I have a bottle of Amrita."), (255, "I do not have either.")]),
            )
        }
    } else if npc_id == FORTUNETELLER_ID {
        (
            "I can tell your future. Would you like me to tell you what awaits you?".to_owned(),
            options(&[(1, "Tell me"), (255, "I prefer the mystery.")]),
        )
    } else if npc_id == MAGGIE_ID {
        if has_all_tokens(character) {
            (
                "Wow. I can't believe you were able to obtain all six tokens. I will use these tokens to teleport you to the life tatic where you can help some poor soul. When you teleport, the tokens will be formed into a soul jade.Give this jade to a any of LonelyGhost so save the ghosts soul. You will recieve a moonbox for your kindness. Are you ready?".to_owned(),
                options(&[(3, "Yes teleport me"), (255, "Not yet.")]),
            )
        } else {
            (
                format!(
                    "Hello {}. My name is Maggie. I tried to conquer these tatics many years ago and was killed. I chose to gaurd the entrance in the afterlife to stop others from following in my footsteps. If you chose to ignore my warning, I will send you in. I cannot control what tatic I send you to unless you have all tokens.",
                    character.entity().name()
                ),
                options(&[(1, "Tokens?"), (255, "This is too dangerous, I must leave at once.")]),
            )
        }
    } else if let Some(index) = tactic(npc_id, GUARDIAN_GOD_ID) {
        if index == 6 {
            (
                "I am so sorry to tell you that you are in the Death Tatic. There is no way to leave this place besides death.".to_owned(),
                options(&[(255, "Oh my")]),
            )
        } else {
            (
                format!(
                    "You have entered the {} Tatic. If you have not already, kill the monsters until you get a CommandToken. That is the only way you can leave this awful place. Good luck adventurer.",
                    TACTICS[index]
                ),
                options(&[(255, "Thanks")]),
            )
        }
    } else if let Some(index) = tactic(npc_id, GHOST_ID) {
        if index == 6 {
            (
                "I am so sorry but I cannot help you. You must die like i did in this tatic.".to_owned(),
                options(&[(255, "Oh my")]),
            )
        } else if character.count_items(FIRST_TOKEN + index as u32) > 0 {
            (
                "Since you have a token to this tatic, I am able to harness its power to send you out. Are you ready? ".to_owned(),
                options(&[(1, "Yes, send me out"), (255, "Thanks")]),
            )
        } else {
            (
                "I wish I could help you adventurer but I need the token the monsters are holding in order to send you out.".to_owned(),
                options(&[(255, "I will get one at once")]),
            )
        }
    } else if (LONELY_GHOST_ID..LONELY_GHOST_ID + LONELY_GHOSTS).contains(&npc_id) {
        if character.count_items(SOUL_JADE) > 0 {
            (
                "Can it be? Is that... YES!!! it's a SoulJade. I thought I would never see one of those. Please free me from this prision so I can join my family. I will give you this MoonBox I acquired before I died. Will you help me?".to_owned(),
                options(&[(1, "Yes, (use SoulJade)"), (255, "Sorry I cannoy help you yet")]),
            )
        } else {
            (
                "You cannot help us without a SoulJade. Please leave here until you get one.".to_owned(),
                options(&[(255, "I will get one at once")]),
            )
        }
    } else {
        return None;
    };
    Some((text, options, face(npc_id)))
}

async fn send(actor: &Actor<ActorState>, npc_id: u32, text: impl Into<String>, options: Options) -> Result<(), Error> {
    let mut builder = MsgTaskDialog::builder().text(text.into());
    for (option, label) in options {
        builder = builder.with_option(option, label);
    }
    let dialog = builder.and().with_avatar(face(npc_id)).task_id(npc_id).build();
    actor.send_all(dialog).await?;
    Ok(())
}

async fn say(actor: &Actor<ActorState>, npc_id: u32, text: &str, option: &str) -> Result<(), Error> {
    send(actor, npc_id, text, vec![(255, option.to_owned())]).await
}

async fn take(state: &State, actor: &Actor<ActorState>, character: &Character, item_type: u32, count: usize) -> Result<bool, Error> {
    let Some(items) = character.consume_items(state.pool(), item_type, count).await? else {
        return Ok(false);
    };
    for item in items {
        actor.send(MsgItemInfo::remove_inventory_item(item)).await?;
    }
    Ok(true)
}

/// Milly asks you to take her letter to her sister.
async fn milly(state: &State, actor: &Actor<ActorState>, character: &Character, option: u8) -> Result<(), Error> {
    const NPC: u32 = MILLY_ID;
    match option {
        1 => send(
            actor,
            NPC,
            "My sister taught me that song, Her lover, Joe, is a brave and kind man. All said they make a good couple, but he left her.",
            options(&[(2, "What a pity."), (255, "Another old love story.")]),
        )
        .await,
        2 => send(
            actor,
            NPC,
            "My sister believes that Joe still loves her. She stays in Bird Island waiting for him. Can you take my letter to her?",
            options(&[(3, "OK. I shall visit her."), (255, "Just passing by.")]),
        )
        .await,
        3 if character.count_items(LETTER) == 0 => {
            if character.inventory_items().len() >= 40 {
                return say(actor, NPC, "Your inventory is full, please leave a slot to take this letter.", "Ok.").await;
            }
            give(state, actor, character, LETTER).await
        },
        _ => Ok(()),
    }
}

/// Minner takes her sister's letter, sends you to Joe with their love token,
/// and gives a MeteorTear for his engraved meteor.
async fn minner(state: &State, actor: &Actor<ActorState>, character: &Character, option: u8) -> Result<(), Error> {
    const NPC: u32 = MINNER_ID;
    let has_letter = character.count_items(LETTER) > 0;
    match option {
        1 if has_letter => send(
            actor,
            NPC,
            "Thanks. I am so pleased to hear from my sister. I have not seen her for a long time. It is very thoughtful of her. Sigh!",
            options(&[(2, "Why are you so sad?"), (255, "I had better leave now.")]),
        )
        .await,
        1 => say(
            actor,
            NPC,
            "I miss you like crazy every time of every day. I love you deeply all my life. Why are you so cruel to break my heart.",
            "I got to go.",
        )
        .await,
        2 => send(
            actor,
            NPC,
            "My lover has left me without any reason. I tried to find him but got no news since he left. Can you help me to find him?",
            options(&[(4, "OK. I shall help you."), (3, "Sorry, I am too busy.")]),
        )
        .await,
        3 => {
            take(state, actor, character, LETTER, 1).await?;
            say(actor, NPC, "Thank you for the letter. Good bye.", "See you.").await
        },
        4 => send(
            actor,
            NPC,
            "Thanks. Joe gave me this bag as our love token. Give it to him when you find him and tell him I am always here waiting for him.",
            options(&[(5, "I shall try my best!")]),
        )
        .await,
        5 if has_letter => {
            if take(state, actor, character, LETTER, 1).await? {
                give(state, actor, character, LOVE_BAG).await?;
            }
            Ok(())
        },
        6 => send(
            actor,
            NPC,
            "Joe! You have seen him? Where is he now? Is everything going on well with him?",
            options(&[(7, "Do not cry."), (255, "Do not ruin the meteor!")]),
        )
        .await,
        7 => send(
            actor,
            NPC,
            "It is said that true love can move the meteor to cry and turn it to meteor tear. I am so glad that Joe does love me.",
            options(&[(8, "All will be fine."), (255, "Return my meteor.")]),
        )
        .await,
        8 => send(
            actor,
            NPC,
            "Joe, Can you hear me? I love you. wherever you go, whatever you do, I will be right here waiting for you.",
            options(&[(9, "Sad love story. Sigh.")]),
        )
        .await,
        9 => send(
            actor,
            NPC,
            "Since Joe enjoys travelling round the world, I only wish he is happy. I shall be very delighted Whenever he thinks of me.",
            options(&[(10, "How about you then?")]),
        )
        .await,
        10 => send(
            actor,
            NPC,
            "It is said a star in the sky represents a person on the earth. When I miss him, I can look at his star and it will twinkle.",
            options(&[(11, "I hope Joe will return.")]),
        )
        .await,
        11 => {
            if take(state, actor, character, SAD_METEOR, 1).await? {
                give(state, actor, character, METEOR_TEAR).await?;
                say(
                    actor,
                    NPC,
                    "Thanks for your news. Please take this meteor tear. I hope you are happy all your life.",
                    "Thanks.",
                )
                .await?;
            }
            Ok(())
        },
        _ => Ok(()),
    }
}

/// Joe engraves his love on a Meteor for Minner, for her love token and an
/// Amrita.
async fn joe(state: &State, actor: &Actor<ActorState>, character: &Character, option: u8) -> Result<(), Error> {
    const NPC: u32 = JOE_ID;
    let ready = [AMRITA, METEOR, LOVE_BAG].iter().all(|item| character.count_items(*item) > 0);
    match option {
        1 if ready => send(
            actor,
            NPC,
            "Thank you. You are very kind. I often have no food and water when I travel around. Where did you get this guardian star?",
            options(&[(2, "It is from Minner."), (7, "I picked it on the road.")]),
        )
        .await,
        1 => say(
            actor,
            NPC,
            "I cannot take your Amrita. There is little water in the vast desert. You may feel thirsty and need it later. And I need a meteor too.",
            "See you then.",
        )
        .await,
        2 => send(
            actor,
            NPC,
            "So she sent you to look for me. I have left so long. She must be heartbroken.",
            options(&[(3, "Why did you leave her?")]),
        )
        .await,
        3 => send(
            actor,
            NPC,
            "We love each other deeply. I wish I could stay with her, but my life dream is to travel around. I cannot give her a warm home.",
            options(&[(4, "You are right."), (255, "I like it very much.")]),
        )
        .await,
        4 => send(
            actor,
            NPC,
            "What a nice meteor you have. I have promised Minner that I shall give her a meteor. Would you please give your meteor to me.",
            options(&[(5, "Here you are.")]),
        )
        .await,
        5 => send(
            actor,
            NPC,
            "Thanks. I shall engrave my love on this meteor. Please kindly give it to Minner. She will understand why I leave her.",
            options(&[(6, "I shall give it to her."), (255, "Do not touch my meteor.")]),
        )
        .await,
        6 if ready => {
            if take(state, actor, character, LOVE_BAG, 1).await? && take(state, actor, character, METEOR, 1).await? {
                give(state, actor, character, SAD_METEOR).await?;
            }
            say(
                actor,
                NPC,
                "Thanks a lot. Please kindly give this meteor to Minner. I hope she can understand me and wish her happy all her life.",
                "I hope so.",
            )
            .await
        },
        6 => say(actor, NPC, "You don't have the items I need!", "Sorry.").await,
        7 => say(
            actor,
            NPC,
            "I must have hurt her a lot. She used to carry this bag wherever she goes. I hope she can forget me and be happy all her life.",
            "I got to go. Bye.",
        )
        .await,
        _ => Ok(()),
    }
}

async fn give(state: &State, actor: &Actor<ActorState>, character: &Character, item_type: u32) -> Result<(), Error> {
    if let Some(item) = character.grant_item(state.pool(), state.shop_catalog(), item_type, false).await? {
        actor.send(MsgItemInfo::inventory_item(item)).await?;
    }
    Ok(())
}

async fn teleport(state: &State, character: &Character, (map_id, x, y): (u32, u16, u16)) -> Result<(), Error> {
    character.teleport(state, map_id, (x, y)).await
}

/// A random tactic, 5% of the time the Death Tactic (Redux Maggie).
fn random_tactic(roll: u32) -> u32 {
    if roll % 100 < 5 {
        return DEATH_TACTIC;
    }
    let number = (roll / 100) % 101;
    match number {
        0..=16 => 1_043,
        17..=32 => 1_044,
        33..=48 => 1_045,
        49..=64 => 1_046,
        65..=80 => 1_047,
        _ => 1_048,
    }
}

/// Handles an answer to one of these NPCs. Returns false for other NPCs.
pub async fn answer(
    state: &State,
    actor: &Actor<ActorState>,
    character: &Character,
    npc_id: u32,
    option: u8,
) -> Result<bool, Error> {
    if npc_id == SIMON_ID {
        simon(state, actor, character, option).await?;
    } else if npc_id == MILLY_ID {
        milly(state, actor, character, option).await?;
    } else if npc_id == MINNER_ID {
        minner(state, actor, character, option).await?;
    } else if npc_id == JOE_ID {
        joe(state, actor, character, option).await?;
    } else if let Some(&(_, token, map_id, x, y, name)) = EXPLORERS.iter().find(|explorer| explorer.0 == npc_id) {
        if option == 1 {
            if take(state, actor, character, token, 1).await? {
                teleport(state, character, (map_id, x, y)).await?;
            } else {
                say(actor, npc_id, &format!("You don't have an {name}!"), "Nevermind...").await?;
            }
        }
    } else if npc_id == LAST_EXPLORER_ID {
        if option == 1 {
            teleport(state, character, TWIN_CITY).await?;
        }
    } else if npc_id == FORTUNETELLER_ID {
        // Redux leaves this as a TODO; the fortune is the Moon Box quest.
        if option == 1 {
            teleport(state, character, MOON_BOX_ENTRY).await?;
        }
    } else if npc_id == MAGGIE_ID {
        maggie(state, actor, character, option).await?;
    } else if let Some(index) = tactic(npc_id, GHOST_ID) {
        if option == 1 && index < 6 && character.count_items(FIRST_TOKEN + index as u32) > 0 {
            teleport(state, character, MOON_BOX_ENTRY).await?;
        }
    } else if (LONELY_GHOST_ID..LONELY_GHOST_ID + LONELY_GHOSTS).contains(&npc_id) {
        if option == 1 {
            if take(state, actor, character, SOUL_JADE, 1).await? {
                teleport(state, character, LIFE_TACTIC_EXIT).await?;
                give(state, actor, character, MOONBOX).await?;
            } else {
                say(
                    actor,
                    npc_id,
                    "You cannot help us without a SoulJade. Please leave here until you get one.",
                    "I will get one at once",
                )
                .await?;
            }
        }
    } else if tactic(npc_id, GUARDIAN_GOD_ID).is_none() {
        return Ok(false);
    }
    Ok(true)
}

async fn simon(state: &State, actor: &Actor<ActorState>, character: &Character, option: u8) -> Result<(), Error> {
    match option {
        1 => {
            if character.spend_virtue(LABYRINTH_VIRTUE) {
                character.save(state).await?;
                teleport(state, character, LABYRINTH).await
            } else {
                say(actor, SIMON_ID, "Sorry, you do not have enough VirtuePoints to enter!", "I see.").await
            }
        },
        5 => {
            send(
                actor,
                SIMON_ID,
                "What diamonds you wish to exchange?",
                options(&[
                    (6, "15 SunDiamonds"),
                    (7, "12 MoonDiamonds"),
                    (8, "12 StarDiamonds"),
                    (9, "10 CloudDiamonds"),
                    (255, "Just passing by."),
                ]),
            )
            .await
        },
        _ => {
            let Some(&(_, diamond, count, reward, silver)) = DIAMONDS.iter().find(|trade| trade.0 == option) else {
                return Ok(());
            };
            if !take(state, actor, character, diamond, count).await? {
                return say(actor, SIMON_ID, "Sorry, you do not have enough diamonds!", "I see.").await;
            }
            give(state, actor, character, reward).await?;
            if silver > 0 {
                character.grant_silver(state.pool(), silver).await?;
                actor.send(MsgUserInfo::from(character)).await?;
            }
            crate::audit::economic("simon_diamonds", character.id(), SIMON_ID, diamond, count as u32);
            Ok(())
        },
    }
}

async fn maggie(state: &State, actor: &Actor<ActorState>, character: &Character, option: u8) -> Result<(), Error> {
    match option {
        1 => {
            send(
                actor,
                MAGGIE_ID,
                "There are six different tokens you must gain from killing monsters. The monsters in each tatic will drop a different token. Once you have all six, I can can channel them for you and teleport you to the life tatic where you can help save some poor souls. I should warn you, there is one more tatic you can be sent to called the Death Tatic. Like the name suggests, if you get sent their, you will die. Good luck adventurer.",
                options(&[(2, "Send me to a tatic"), (255, "This is too dangerous, I must leave at once.")]),
            )
            .await
        },
        2 => {
            let map_id = random_tactic(crate::combat::next_roll());
            teleport(state, character, (map_id, TACTIC_ENTRY.0, TACTIC_ENTRY.1)).await
        },
        3 if has_all_tokens(character) => {
            for i in 0..6 {
                if !take(state, actor, character, FIRST_TOKEN + i, 1).await? {
                    return Ok(());
                }
            }
            give(state, actor, character, SOUL_JADE).await?;
            teleport(state, character, (LIFE_TACTIC, TACTIC_ENTRY.0, TACTIC_ENTRY.1)).await
        },
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tactics_follow_maggie_odds() {
        assert_eq!(random_tactic(3), DEATH_TACTIC);
        assert_eq!(random_tactic(10 * 100 + 50), 1_043);
        assert_eq!(random_tactic(100 * 100 + 50), 1_048);
        let maps = (0..10_000).map(random_tactic).collect::<std::collections::HashSet<_>>();
        assert_eq!(maps.len(), 7);
    }

    #[test]
    fn tactic_npcs_resolve_their_index() {
        assert_eq!(tactic(6_110, GUARDIAN_GOD_ID), Some(0));
        assert_eq!(tactic(6_126, GHOST_ID), Some(6));
        assert_eq!(tactic(6_127, GHOST_ID), None);
        assert_eq!(tactic(6_100, GHOST_ID), None);
    }
}
