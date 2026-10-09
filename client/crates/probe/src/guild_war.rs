//! `probe --guild-war <auth address>`: the Guild War played through by
//! test accounts against a running server (one started with
//! `GUILD_WAR_POLE_LIFE` and `GUILD_WAR_GATE_LIFE` small, so a round ends in
//! minutes). The accounts must be game masters (`accounts.gm`): the test
//! uses $gw, $tele, $level, $silver and $create-guild.
//!
//! Three characters: the leader of GwAlpha, a member of GwAlpha, and the
//! leader of GwBeta. Each check prints PASS or FAIL; the run ends with the
//! count.

use gravak_protocol::{
    ActionPacket, AuthResponse, GameEvent, GameSession, GuildPanelPacket, InteractionPacket, NpcDialogPacket,
    NpcPacket, RegisterCharacter, TalkPacket, WalkPacket, authenticate, register_account,
};
use tokio::time::{Duration, Instant, sleep, timeout_at};

const GUILD_MAP: u32 = 1_038;
const POLE: u32 = 100_810;
const LEFT_GATE: u32 = 108_855;
const PRISON: u32 = 6_001;
const WARDEN: u32 = 140;
const CONDUCTRESS: u32 = 815;
const LIFE: u32 = 0;
/// Where the pole and the left gate stand.
const POLE_AT: (u16, u16) = (84, 99);
const LEFT_GATE_AT: (u16, u16) = (163, 210);
/// The left gate's closed and open looks.
const GATE_CLOSED: u16 = 241;
/// The way through the left gate, from outside it at (166, 215): up its
/// passage, which bends north-west (4 north, 3 north-west).
const THROUGH_LEFT_GATE: [u8; 11] = [4, 4, 4, 4, 3, 4, 3, 4, 3, 4, 4];
const BEFORE_LEFT_GATE: (u16, u16) = (166, 215);
const GATE_OPEN: u16 = 251;

type Result<T> = std::result::Result<T, String>;

struct Bot {
    session: GameSession,
    id: u32,
    name: String,
    map: u32,
    at: (u16, u16),
    silver: u32,
    /// The guild the server says this character belongs to.
    guild: Option<u32>,
}

impl Bot {
    async fn login(address: &str, username: &str, password: &str, character: &str) -> Result<Self> {
        match register_account(address, username, password).await {
            Ok(result) => println!("  account {username}: {result:?}"),
            Err(error) => println!("  account {username}: {error}"),
        }
        for attempt in 0..2 {
            let server = match authenticate(address, username, password, "Gravak").await.map_err(|e| e.to_string())? {
                AuthResponse::Forwarded(server) => server,
                AuthResponse::Rejected(rejection) => return Err(format!("{username}: login rejected: {}", rejection.message)),
            };
            let game = format!("{}:{}", server.address, server.port);
            let mut session = GameSession::connect(&game, server.token, 5065, "English").await.map_err(|e| e.to_string())?;
            loop {
                match session.next_event().await.map_err(|e| e.to_string())? {
                    Some(GameEvent::UserInfo(info)) => {
                        return Ok(Self {
                            session,
                            id: info.character_id,
                            name: info.character_name,
                            map: 0,
                            at: (0, 0),
                            silver: info.silver,
                            guild: None,
                        });
                    },
                    Some(GameEvent::Talk(talk)) if talk.message == "NEW_ROLE" && attempt == 0 => {
                        session
                            .send(&RegisterCharacter::new(username, character, server.token as u32))
                            .await
                            .map_err(|e| e.to_string())?;
                        let _ = session.next_event().await;
                        break;
                    },
                    Some(_) => {},
                    None => return Err(format!("{username}: the game server closed the session")),
                }
            }
        }
        Err(format!("{username}: no character after registering one"))
    }

    fn note(&mut self, event: &GameEvent) {
        match event {
            GameEvent::Action(action) if action.action_type == 86 || action.action_type == 74 => {
                if action.action_type == 86 {
                    self.map = action.data1;
                }
                self.at = ((action.data2 & 0xffff) as u16, (action.data2 >> 16) as u16);
            },
            // The map itself (the prison is 6001), not the map file it uses.
            GameEvent::MapInfo(info) => self.map = info.uid,
            GameEvent::Guild(guild) if guild.action == 14 => self.guild = (guild.data != 0).then_some(guild.data),
            GameEvent::UserInfo(info) if info.character_id == self.id => self.silver = info.silver,
            GameEvent::Walk(walk) if walk.character_id == self.id => {
                const DX: [i32; 8] = [0, -1, -1, -1, 0, 1, 1, 1];
                const DY: [i32; 8] = [1, 1, 0, -1, -1, -1, 0, 1];
                let direction = usize::from(walk.direction % 8);
                self.at = ((i32::from(self.at.0) + DX[direction]) as u16, (i32::from(self.at.1) + DY[direction]) as u16);
            },
            _ => {},
        }
    }

    /// Events for `wait`, all of them.
    async fn collect(&mut self, wait: Duration) -> Result<Vec<GameEvent>> {
        let deadline = Instant::now() + wait;
        let mut events = Vec::new();
        while let Ok(event) = timeout_at(deadline, self.session.next_event()).await {
            match event.map_err(|e| e.to_string())? {
                Some(event) => {
                    self.note(&event);
                    events.push(event);
                },
                None => return Err(format!("{}: session closed", self.name)),
            }
        }
        Ok(events)
    }

    /// The first event `wanted` accepts within `wait`.
    async fn until(&mut self, wait: Duration, mut wanted: impl FnMut(&GameEvent) -> bool) -> Result<Option<GameEvent>> {
        let deadline = Instant::now() + wait;
        while let Ok(event) = timeout_at(deadline, self.session.next_event()).await {
            match event.map_err(|e| e.to_string())? {
                Some(event) => {
                    self.note(&event);
                    if wanted(&event) {
                        return Ok(Some(event));
                    }
                },
                None => return Err(format!("{}: session closed", self.name)),
            }
        }
        Ok(None)
    }

    async fn say(&mut self, line: &str) -> Result<()> {
        self.session.send(&TalkPacket::public(self.id, &self.name, line)).await.map_err(|e| e.to_string())
    }

    /// A $ command, and its answer lines for a moment.
    async fn command(&mut self, line: &str) -> Result<Vec<String>> {
        self.say(line).await?;
        Ok(talks(&self.collect(Duration::from_millis(600)).await?))
    }

    async fn tele(&mut self, map: u32, x: u16, y: u16) -> Result<()> {
        self.say(&format!("$tele {map} {x} {y}")).await?;
        self.until(Duration::from_secs(3), |event| matches!(event, GameEvent::Action(a) if a.action_type == 86))
            .await?
            .ok_or_else(|| format!("{}: no teleport to {map} {x},{y}", self.name))?;
        self.collect(Duration::from_millis(400)).await?;
        Ok(())
    }

    async fn attack(&mut self, target: u32, (x, y): (u16, u16)) -> Result<()> {
        self.session
            .send(&InteractionPacket::attack(self.id, target, x, y))
            .await
            .map_err(|e| e.to_string())
    }

    /// The exit's Exercise: whether the server says the hero went, its
    /// lines, and the map the hero is on after.
    async fn exercise(&mut self) -> Result<(Option<bool>, Vec<String>, u32)> {
        self.session.send(&ActionPacket::exercise(self.id)).await.map_err(|e| e.to_string())?;
        let events = self.collect(Duration::from_millis(900)).await?;
        let went = events.iter().find_map(|event| match event {
            GameEvent::Action(action) if action.action_type == ActionPacket::EXERCISE => Some(action.data1 == 1),
            _ => None,
        });
        Ok((went, talks(&events), self.map))
    }

    async fn walk(&mut self, direction: u8) -> Result<()> {
        self.session.send(&WalkPacket::new(self.id, direction)).await.map_err(|e| e.to_string())
    }

    /// Walks `steps`; whether a step was refused, and where it ended.
    async fn walk_path(&mut self, steps: &[u8]) -> Result<bool> {
        for direction in steps {
            self.walk(*direction).await?;
            let lines = talks(&self.collect(Duration::from_millis(350)).await?);
            if lines.iter().any(|line| line.contains("Invalid Location")) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn talk_to(&mut self, npc: u32) -> Result<Vec<String>> {
        self.session.send(&NpcPacket::activate(npc, 0)).await.map_err(|e| e.to_string())?;
        Ok(dialog(&self.collect(Duration::from_millis(700)).await?))
    }

    async fn answer(&mut self, npc: u32, option: u8) -> Result<Vec<GameEvent>> {
        self.session.send(&NpcDialogPacket::answer(npc, option)).await.map_err(|e| e.to_string())?;
        self.collect(Duration::from_millis(900)).await
    }
}

fn talks(events: &[GameEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            GameEvent::Talk(talk) => Some(talk.message.clone()),
            _ => None,
        })
        .collect()
}

fn dialog(events: &[GameEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            GameEvent::TaskDialog(dialog) => Some(dialog.messages.join(" | ")),
            _ => None,
        })
        .collect()
}

fn life_of(events: &[GameEvent], id: u32) -> Option<u64> {
    events.iter().rev().find_map(|event| match event {
        GameEvent::UserAttrib(attrib) if attrib.character_id == id && attrib.attribute == LIFE => Some(attrib.value),
        _ => None,
    })
}

fn look_of(events: &[GameEvent], id: u32) -> Option<(u16, Option<String>)> {
    events.iter().rev().find_map(|event| match event {
        GameEvent::NpcInfo(info) if info.id == id => Some((info.look, info.name.clone())),
        _ => None,
    })
}

struct Report {
    passed: usize,
    failed: Vec<String>,
}

impl Report {
    fn check(&mut self, name: &str, ok: bool, detail: impl std::fmt::Display) {
        if ok {
            self.passed += 1;
            println!("PASS  {name}: {detail}");
        } else {
            self.failed.push(name.to_owned());
            println!("FAIL  {name}: {detail}");
        }
    }
}

/// Every bot reads what came to it, so no socket fills while another works.
async fn settle(bots: &mut [&mut Bot], wait: Duration) -> Result<Vec<Vec<GameEvent>>> {
    let mut all = Vec::new();
    for bot in bots.iter_mut() {
        all.push(bot.collect(wait).await?);
    }
    Ok(all)
}

/// Hits `target` with every bot in turn until its life is at most `until`
/// or `rounds` go by; the life last seen, the damage of one hit, and every
/// event each bot read.
async fn beat(
    bots: &mut [&mut Bot],
    target: u32,
    at: (u16, u16),
    until: u64,
    rounds: usize,
) -> Result<(Option<u64>, Option<u32>, Vec<GameEvent>)> {
    let mut life = None;
    let mut damage = None;
    let mut seen = Vec::new();
    for _ in 0..rounds {
        for bot in bots.iter_mut() {
            bot.attack(target, at).await?;
        }
        for bot in bots.iter_mut() {
            let events = bot.collect(Duration::from_millis(350)).await?;
            if let Some(value) = life_of(&events, target) {
                life = Some(value);
            }
            damage = damage.or_else(|| {
                events.iter().find_map(|event| match event {
                    GameEvent::Interaction(hit) if hit.target_id == target && hit.value > 0 => Some(hit.value),
                    _ => None,
                })
            });
            seen.extend(events);
        }
        if life.is_some_and(|life| life <= until) {
            break;
        }
    }
    Ok((life, damage, seen))
}

pub async fn run(address: &str) -> std::result::Result<(), String> {
    let mut report = Report { passed: 0, failed: Vec::new() };
    println!("== Logging in the test characters");
    let mut alpha = Bot::login(address, "gwtesta1", "gwtest123", "GwAlphaLead").await?;
    let mut mate = Bot::login(address, "gwtesta2", "gwtest123", "GwAlphaMate").await?;
    let mut beta = Bot::login(address, "gwtestb1", "gwtest123", "GwBetaLead").await?;
    for bot in [&mut alpha, &mut mate, &mut beta] {
        println!("  {} (id {}) silver {}", bot.name, bot.id, bot.silver);
        let gm = bot.command("$level 130").await?;
        if gm.iter().any(|line| line.contains("for game masters")) || gm.is_empty() {
            return Err(format!("{} is not a game master: {gm:?}", bot.name));
        }
        bot.command("$silver 5000000").await?;
        bot.command("$test-health").await?;
    }

    println!("== Guilds");
    for (bot, name) in [(&mut alpha, "GwAlpha"), (&mut beta, "GwBeta")] {
        if bot.guild.is_none() {
            let lines = bot.command(&format!("$create-guild {name}")).await?;
            println!("  {name}: {lines:?}");
        } else {
            println!("  {} already leads guild {:?}", bot.name, bot.guild);
        }
    }
    if mate.guild.is_some() {
        println!("  {} is already in guild {:?}", mate.name, mate.guild);
    } else {
    let guild = alpha.guild.ok_or("GwAlphaLead has no guild")?;
    let invite = GuildPanelPacket::new(GuildPanelPacket::INVITE, 0, 0).with_texts([&mate.name, "", "", ""]);
    alpha.session.send(&invite).await.map_err(|e| e.to_string())?;
    println!("  invite: {:?}", talks(&alpha.collect(Duration::from_millis(500)).await?));
    let invited = mate.collect(Duration::from_millis(500)).await?;
    println!("  {} is told: {:?}", mate.name, talks(&invited));
    mate.session
        .send(&GuildPanelPacket::new(GuildPanelPacket::ACCEPT_INVITE, guild, 0))
        .await
        .map_err(|e| e.to_string())?;
    let joined = talks(&mate.collect(Duration::from_millis(800)).await?);
    println!("  {} joins GwAlpha: {joined:?}", mate.name);
    if mate.guild != Some(guild) {
        return Err(format!("{} did not join GwAlpha (guild {:?})", mate.name, mate.guild));
    }
    }
    settle(&mut [&mut alpha, &mut beta], Duration::from_millis(300)).await?;

    println!("== Before the war");
    alpha.command("$gw end").await?;
    settle(&mut [&mut mate, &mut beta], Duration::from_millis(300)).await?;
    alpha.tele(GUILD_MAP, POLE_AT.0 + 1, POLE_AT.1 + 1).await?;
    alpha.attack(POLE, POLE_AT).await?;
    let lines = talks(&alpha.collect(Duration::from_millis(600)).await?);
    report.check(
        "the pole cannot be hit outside the war",
        lines.iter().any(|line| line.contains("not running")),
        format!("{lines:?}"),
    );

    println!("== The war starts");
    alpha.say("$gw start").await?;
    let mut announced = 0;
    for bot in [&mut alpha, &mut mate, &mut beta] {
        let lines = talks(&bot.collect(Duration::from_millis(900)).await?);
        announced += usize::from(lines.iter().any(|line| line.contains("Guild War has begun")));
    }
    report.check("everyone online hears the war begin", announced == 3, format!("{announced} of 3"));

    println!("== The left gate");
    beta.tele(GUILD_MAP, BEFORE_LEFT_GATE.0, BEFORE_LEFT_GATE.1).await?;
    println!("  {} at {:?} on {}", beta.name, beta.at, beta.map);
    let start = beta.at;
    let refused = beta.walk_path(&THROUGH_LEFT_GATE).await?;
    report.check(
        "a closed gate stops whoever walks into it",
        refused && beta.at.1 > LEFT_GATE_AT.1 + 3,
        format!("refused={refused}, from {start:?} to {:?}", beta.at),
    );
    let (life, damage, events) = beat(&mut [&mut beta], LEFT_GATE, LEFT_GATE_AT, 0, 3).await?;
    println!("  damage of one hit on the gate: {damage:?}, its life {life:?}");
    report.check("a gate takes damage in the war", life.is_some_and(|life| life > 1), format!("life {life:?}"));
    let silver_before = beta.silver;
    let _ = events;

    println!("== The pole");
    mate.tele(GUILD_MAP, POLE_AT.0 - 1, POLE_AT.1 + 1).await?;
    beta.tele(GUILD_MAP, POLE_AT.0 + 1, POLE_AT.1 - 1).await?;
    let alpha_silver = alpha.silver;
    let (life, damage, events) = beat(&mut [&mut alpha, &mut mate, &mut beta], POLE, POLE_AT, 0, 2).await?;
    println!("  damage of one hit on the pole: {damage:?}, its life {life:?}");
    report.check("the pole takes damage in the war", life.is_some(), format!("life {life:?}"));
    let _ = events;
    alpha.collect(Duration::from_millis(400)).await?;
    // A hit pays a 250th of its damage in silver: nothing under 250.
    let pays = damage.map_or(0, |damage| damage / 250);
    report.check(
        "hitting the pole pays a 250th of the damage in silver",
        (pays == 0 && alpha.silver == alpha_silver) || alpha.silver > alpha_silver,
        format!("{} -> {} ({pays} a hit)", alpha_silver, alpha.silver),
    );
    let _ = silver_before;

    // The score table comes every ten seconds.
    let table = alpha
        .until(Duration::from_secs(12), |event| matches!(event, GameEvent::Talk(t) if t.message.starts_with("No 1:")))
        .await?;
    report.check(
        "the score table is shown on the map",
        table.is_some(),
        table.map_or_else(|| "none".to_owned(), |event| format!("{:?}", talks(&[event]))),
    );

    // GwAlpha hits twice as often: two of its members against one of GwBeta.
    println!("== Felling the pole");
    let (life, _, events) = beat(&mut [&mut alpha, &mut mate, &mut beta], POLE, POLE_AT, 0, 400).await?;
    let mut all = events;
    all.extend(alpha.collect(Duration::from_millis(800)).await?);
    let victory = talks(&all).into_iter().find(|line| line.contains("emerged victorious"));
    report.check("the pole falls and a guild wins the round", life == Some(0) && victory.is_some(), format!("{victory:?}"));
    report.check(
        "GwAlpha, with the best score, wins",
        victory.as_deref().is_some_and(|line| line.starts_with("GwAlpha")),
        format!("{victory:?}"),
    );
    let pole = look_of(&all, POLE);
    report.check(
        "the pole carries the winner's name",
        pole.as_ref().is_some_and(|(_, name)| name.as_deref() == Some("GwAlpha")),
        format!("{pole:?}"),
    );
    alpha.attack(POLE, POLE_AT).await?;
    let lines = talks(&alpha.collect(Duration::from_millis(500)).await?);
    report.check(
        "no hits between rounds",
        lines.iter().any(|line| line.contains("new round")),
        format!("{lines:?}"),
    );
    let reset = alpha
        .until(Duration::from_secs(13), |event| {
            matches!(event, GameEvent::UserAttrib(a) if a.character_id == POLE && a.attribute == LIFE && a.value > 0)
        })
        .await?;
    report.check(
        "a new round starts with the pole whole again",
        reset.is_some(),
        format!("{reset:?}"),
    );
    settle(&mut [&mut mate, &mut beta], Duration::from_millis(200)).await?;

    println!("== The gates and the winners");
    beta.tele(GUILD_MAP, LEFT_GATE_AT.0, LEFT_GATE_AT.1 + 5).await?;
    let lines = beta.talk_to(LEFT_GATE).await?;
    report.check("a gate does not talk to other guilds", lines.is_empty(), format!("{lines:?}"));
    mate.tele(GUILD_MAP, LEFT_GATE_AT.0, LEFT_GATE_AT.1 + 5).await?;
    let lines = mate.talk_to(LEFT_GATE).await?;
    report.check(
        "a gate offers the winners a way in",
        lines.iter().any(|line| line.contains("Let me in")),
        format!("{lines:?}"),
    );
    let events = mate.answer(LEFT_GATE, 2).await?;
    let lines = dialog(&events);
    report.check(
        "only the winners' leader opens a gate",
        lines.iter().any(|line| line.contains("not able")),
        format!("{lines:?}"),
    );
    let events = mate.answer(LEFT_GATE, 1).await?;
    let inside = mate.at;
    report.check(
        "Let me in carries a winner inside",
        mate.map == GUILD_MAP && inside.1 < LEFT_GATE_AT.1,
        format!("at {inside:?} ({} events)", events.len()),
    );
    alpha.tele(GUILD_MAP, LEFT_GATE_AT.0, LEFT_GATE_AT.1 + 5).await?;
    alpha.talk_to(LEFT_GATE).await?;
    let events = alpha.answer(LEFT_GATE, 2).await?;
    let look = look_of(&events, LEFT_GATE);
    report.check(
        "the leader opens the gate",
        look.as_ref().is_some_and(|(look, _)| *look == GATE_OPEN),
        format!("{look:?}"),
    );
    beta.tele(GUILD_MAP, BEFORE_LEFT_GATE.0, BEFORE_LEFT_GATE.1).await?;
    let start = beta.at;
    let refused = beta.walk_path(&THROUGH_LEFT_GATE).await?;
    report.check(
        "an open gate lets anyone through",
        !refused && beta.at.1 + 3 < LEFT_GATE_AT.1,
        format!("from {start:?} to {:?}", beta.at),
    );
    alpha.talk_to(LEFT_GATE).await?;
    let events = alpha.answer(LEFT_GATE, 2).await?;
    let look = look_of(&events, LEFT_GATE);
    report.check(
        "the leader closes it again",
        look.as_ref().is_some_and(|(look, _)| *look == GATE_CLOSED),
        format!("{look:?}"),
    );

    println!("== Breaking and repairing a gate");
    beta.tele(GUILD_MAP, LEFT_GATE_AT.0, LEFT_GATE_AT.1 + 4).await?;
    alpha.collect(Duration::from_millis(200)).await?;
    let (life, _, events) = beat(&mut [&mut beta], LEFT_GATE, LEFT_GATE_AT, 1, 400).await?;
    let mut all = events;
    all.extend(alpha.collect(Duration::from_millis(600)).await?);
    let look = look_of(&all, LEFT_GATE);
    report.check(
        "a gate beaten to its last point falls open",
        life == Some(1) && look.as_ref().is_some_and(|(look, _)| *look == GATE_OPEN),
        format!("life {life:?}, look {look:?}"),
    );
    beta.attack(LEFT_GATE, LEFT_GATE_AT).await?;
    let lines = talks(&beta.collect(Duration::from_millis(500)).await?);
    report.check("a fallen gate takes no more hits", lines.iter().any(|line| line.contains("open")), format!("{lines:?}"));
    let lines = alpha.talk_to(LEFT_GATE).await?;
    report.check(
        "the winners are offered a repair",
        lines.iter().any(|line| line.contains("Repair")),
        format!("{lines:?}"),
    );
    let silver = alpha.silver;
    let events = alpha.answer(LEFT_GATE, 3).await?;
    let look = look_of(&events, LEFT_GATE);
    let repaired = life_of(&events, LEFT_GATE);
    report.check(
        "the repair closes the gate and costs 2,000,000 silver",
        look.as_ref().is_some_and(|(look, _)| *look == GATE_CLOSED) && silver.saturating_sub(alpha.silver) == 2_000_000,
        format!("look {look:?}, life {repaired:?}, silver {silver} -> {}", alpha.silver),
    );

    println!("== The conductresses");
    beta.tele(GUILD_MAP, 333, 340).await?;
    alpha.tele(GUILD_MAP, 333, 341).await?;
    let silver = beta.silver;
    beta.talk_to(CONDUCTRESS).await?;
    beta.answer(CONDUCTRESS, 1).await?;
    beta.collect(Duration::from_millis(500)).await?;
    report.check(
        "others pay 1,000 silver to the conductress",
        silver.saturating_sub(beta.silver) == 1_000 && beta.map != GUILD_MAP,
        format!("silver {silver} -> {}, map {}", beta.silver, beta.map),
    );
    let silver = alpha.silver;
    alpha.talk_to(CONDUCTRESS).await?;
    alpha.answer(CONDUCTRESS, 1).await?;
    alpha.collect(Duration::from_millis(500)).await?;
    report.check(
        "the winners ride free",
        alpha.silver == silver && alpha.map != GUILD_MAP,
        format!("silver {silver} -> {}, map {}", alpha.silver, alpha.map),
    );

    println!("== Dying in the war");
    beta.tele(GUILD_MAP, 333, 340).await?;
    mate.tele(GUILD_MAP, 334, 340).await?;
    mate.command("$level 1").await?;
    beta.session.send(&ActionPacket::set_kill_mode(beta.id, 0)).await.map_err(|e| e.to_string())?;
    beta.collect(Duration::from_millis(300)).await?;
    mate.collect(Duration::from_millis(300)).await?;
    let mut dead = false;
    for _ in 0..30 {
        beta.attack(mate.id, mate.at).await?;
        beta.collect(Duration::from_millis(200)).await?;
        let events = mate.collect(Duration::from_millis(250)).await?;
        if events.iter().any(|event| {
            matches!(event, GameEvent::Interaction(hit) if hit.target_id == mate.id && hit.action == 14)
        }) {
            dead = true;
            break;
        }
    }
    report.check("a player can be slain on the guild map", dead, format!("{} dead={dead}", mate.name));
    if dead {
        println!("  waiting out the 20 second revive delay");
        sleep(Duration::from_secs(21)).await;
        settle(&mut [&mut alpha, &mut beta], Duration::from_millis(100)).await?;
        mate.session.send(&ActionPacket::reborn(mate.id, false)).await.map_err(|e| e.to_string())?;
        mate.collect(Duration::from_millis(1200)).await?;
        report.check(
            "the slain revive in the war prison",
            mate.map == PRISON,
            format!("map {} at {:?}", mate.map, mate.at),
        );
        let lines = mate.talk_to(WARDEN).await?;
        report.check(
            "the warden only pardons at :00 and :30 while the war runs",
            lines.iter().any(|line| line.contains("not time for Pardon") || line.contains("time for Pardon")),
            format!("{lines:?}"),
        );
    }
    mate.command("$level 130").await?;

    println!("== The war ends");
    alpha.say("$gw end").await?;
    let mut heard = 0;
    for bot in [&mut alpha, &mut mate, &mut beta] {
        let lines = talks(&bot.collect(Duration::from_millis(900)).await?);
        heard += usize::from(lines.iter().any(|line| line.contains("GwAlpha has won")));
    }
    report.check("everyone hears the winner when the war ends", heard == 3, format!("{heard} of 3"));
    alpha.tele(GUILD_MAP, POLE_AT.0 + 1, POLE_AT.1 + 1).await?;
    alpha.attack(POLE, POLE_AT).await?;
    let lines = talks(&alpha.collect(Duration::from_millis(500)).await?);
    report.check("after the war the pole cannot be hit", lines.iter().any(|line| line.contains("not running")), format!("{lines:?}"));
    for bot in [&mut alpha, &mut mate, &mut beta] {
        bot.tele(1_002, 430, 380).await?;
    }

    println!("\n{} passed, {} failed {:?}", report.passed, report.failed.len(), report.failed);
    if report.failed.is_empty() { Ok(()) } else { Err(format!("{} checks failed", report.failed.len())) }
}

/// `probe --exercise <auth address>`: the exit confirmation's Exercise
/// (MsgAction 9010) against a running server: refused without Heaven's
/// Blessing and from the jail, granted with it (the hero goes to the
/// offline training ground, map 601).
pub async fn run_exercise(address: &str) -> std::result::Result<(), String> {
    let mut report = Report { passed: 0, failed: Vec::new() };
    let mut bot = Bot::login(address, "gwtesta2", "gwtest123", "GwAlphaMate").await?;
    bot.tele(1_002, 430, 380).await?;
    let (went, lines, map) = bot.exercise().await?;
    let without = went == Some(false) && lines.iter().any(|line| line.contains("Heaven's Blessing"));
    // A character blessed by an earlier run goes: then the rest still holds.
    report.check(
        "without Heaven's Blessing, Exercise is refused",
        without || went == Some(true),
        format!("went {went:?}, map {map}, {lines:?}"),
    );
    if went == Some(true) {
        bot.tele(1_002, 430, 380).await?;
    } else {
        bot.say("$item 1200000").await?;
        let item = bot
            .until(Duration::from_secs(2), |event| matches!(event, GameEvent::ItemInfo(item) if item.item_type == 1_200_000))
            .await?;
        let Some(GameEvent::ItemInfo(item)) = item else {
            return Err("no Heaven's Blessing item".to_owned());
        };
        bot.session
            .send(&gravak_protocol::ItemActionPacket::use_item(bot.id, item.item_uid))
            .await
            .map_err(|e| e.to_string())?;
        println!("  blessing: {:?}", talks(&bot.collect(Duration::from_millis(800)).await?));
    }
    bot.tele(6_000, 29, 72).await?;
    let (went, lines, map) = bot.exercise().await?;
    report.check(
        "Exercise is refused in the jail",
        went == Some(false) && map == 6_000,
        format!("went {went:?}, map {map}, {lines:?}"),
    );
    bot.tele(1_002, 430, 380).await?;
    let (went, lines, map) = bot.exercise().await?;
    report.check(
        "with Heaven's Blessing, Exercise sends the hero to offline training",
        went == Some(true) && map == 601,
        format!("went {went:?}, map {map}, {lines:?}"),
    );
    bot.tele(1_002, 430, 380).await?;
    println!("\n{} passed, {} failed {:?}", report.passed, report.failed.len(), report.failed);
    if report.failed.is_empty() { Ok(()) } else { Err(format!("{} checks failed", report.failed.len())) }
}

/// `probe --commands <auth address> <account> <password> <character> <line>...`:
/// logs a test account in, says each line (GM `$` commands among them) and
/// prints what comes back.
pub async fn run_commands(address: &str, account: &str, password: &str, character: &str, lines: &[String]) -> std::result::Result<(), String> {
    let mut bot = Bot::login(address, account, password, character).await?;
    println!("{} (id {}) on map {}", bot.name, bot.id, bot.map);
    for line in lines {
        let answer = bot.command(line).await?;
        println!("> {line}\n  {answer:?}");
    }
    Ok(())
}
