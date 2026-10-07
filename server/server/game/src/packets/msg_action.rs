use super::{MsgTalk, TalkChannel};
use crate::entities::Character;
use crate::packets::{MapFlags, MsgMapInfo, MsgPlayer, MsgUserInfo, MsgWeather};
use crate::state::State;
use crate::systems::TileType;
use crate::{utils, ActorState, Error};
use async_trait::async_trait;
use num_enum::{FromPrimitive, IntoPrimitive};
use primitives::Location;
use serde::{Deserialize, Serialize};
use tq_network::{Actor, PacketID, PacketProcess};
use utils::LoHi;

#[derive(Copy, Clone, Debug, Default, FromPrimitive, IntoPrimitive)]
#[repr(u16)]
pub enum ActionType {
    #[default]
    Unknown = 0,
    SendLocation = 74,
    SendItems = 75,
    SendAssociates = 76,
    SendProficiencies = 77,
    SendSpells = 78,
    ChangeFacing = 79,
    ChangeAction = 81,
    ChangeMap = 85,
    Teleport = 86,
    LevelUp = 92,
    XpClear = 93,
    Reborn = 94,
    DelRole = 95,
    SetKillMode = 96,
    ConfirmGuild = 97,
    Mine = 99,
    /// Not sure of the name...
    BotCheckA = 100,
    QueryEntity = 102,
    /// to client only
    MapARGB = 104,
    QueryTeamMember = 106,
    /// to client idUser is Player ID, unPosX unPosY is Player pos
    KickBack = 108,
    // to client only, data is magic type
    DropMagic = 109,
    // to client only, data is weapon skill type
    DropSkill = 110,
    CreateBooth = 111,
    SuspendBooth = 112,
    ResumeBooth = 113,
    LeaveBooth = 114,
    PostCommand = 116,
    QueryEquipment = 117,
    AbortTransform = 118,
    // CombineSubSyn = 119,
    // idTargetSyn
    TakeOff = 120,
    GetMoney = 121,
    CancelKeepBow = 122,
    QueryEnemyInfo = 123,
    OpenDialog = 126,
    // FlashStatus = 127,
    LoginCompeleted = 130,
    /// Client-side monster resurrection effect.
    SpawnEffect = 131,
    LeaveMap = 132,
    Jump = 133,
    Ghost = 137,
    Synchro = 138,
    QueryFriendInfo = 140,
    // QueryLeaveWord = 141,
    ChangeFace = 142,
}

#[derive(Copy, Clone, Debug, Default, FromPrimitive, IntoPrimitive)]
#[repr(u16)]
pub enum KillMode {
    #[default]
    Free = 0,
    Safe = 1,
    Team = 2,
    Arrestment = 3,
}

/// Message containing a general action being performed by the client. Commonly
/// used as a request-response protocol for question and answer like exchanges.
/// For example, walk requests are responded to with an answer as to if the step
/// is legal or not.
#[derive(Debug, Default, Serialize, Deserialize, Clone, PacketID)]
#[packet(id = 1010)]
pub struct MsgAction {
    pub client_timestamp: u32,
    pub character_id: u32,
    pub data1: u32,
    pub data2: u32,
    pub details: u16,
    pub action_type: u16,
}

impl MsgAction {
    pub fn new(character_id: u32, data1: u32, data2: u32, details: u16, action_type: ActionType) -> Self {
        Self {
            client_timestamp: utils::current_ts(),
            character_id,
            data1,
            data2,
            details,
            action_type: action_type as u16,
        }
    }

    pub fn from_character(character: &Character, data1: u32, action_type: ActionType) -> Self {
        let character_id = character.id();
        let (x, y, d) = character.entity().location().into();
        let data2 = u32::constract(y, x);
        let details = d as u16;
        Self::new(character_id, data1, data2, details, action_type)
    }

    #[tracing::instrument(skip_all)]
    async fn handle_send_location(&self, state: &State, actor: &Actor<ActorState>) -> Result<(), Error> {
        let mut res = self.clone();
        let entity = actor.try_entity()?;
        let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
        let map_id = character.entity().map_id();
        let location = character.entity().location();
        match state.try_map(map_id) {
            Ok(mymap) => {
                res.data1 = map_id;
                res.data2 = u32::constract(location.y, location.x);
                mymap.insert_entity(entity).await?;
                actor.send(res).await?;
                actor.send(MsgMapInfo::from_map(mymap)).await?;
                if !mymap.weather().is_unknwon() {
                    actor.send(MsgWeather::new(mymap.weather())).await?;
                }
                let screen = actor.screen();
                screen.load_surroundings(state).await?;
            },
            Err(_) => {
                tracing::warn!(
                    character_id = character.id(),
                    %map_id,
                    "map not found",
                );
                // Set default map and location.
                let location = Location::new(430, 378, 0);
                character.entity().set_map_id(1002).set_location(location);
                actor
                    .send(MsgTalk::from_system(
                        character.id(),
                        TalkChannel::System,
                        "Map not found, teleporting to Twin City.".to_string(),
                    ))
                    .await?;
                actor.shutdown().await?;
                return Ok(());
            },
        };
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn handle_map_argb(&self, state: &State, actor: &Actor<ActorState>) -> Result<(), Error> {
        let mut res = self.clone();
        let entity = actor.try_entity()?;
        let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
        let map_id = character.entity().map_id();
        let location = character.entity().location();
        let map = state.try_map(map_id)?;
        res.data1 = map.color();
        res.data2 = u32::constract(location.y, location.x);
        actor.send(res).await?;
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn handle_login_completed(&self, _state: &State, actor: &Actor<ActorState>) -> Result<(), Error> {
        let res = self.clone();
        actor.send(res).await?;
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn handle_leave_booth(&self, state: &State, actor: &Actor<ActorState>) -> Result<(), Error> {
        let entity = actor.try_entity()?;
        let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
        leave_booth(state, actor, character).await
    }

    #[tracing::instrument(skip_all)]
    async fn handle_create_booth(&self, state: &State, actor: &Actor<ActorState>) -> Result<(), Error> {
        let entity = actor.try_entity()?;
        let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
        if self.character_id != character.id() {
            return Ok(());
        }
        let map = state.try_map(character.entity().map_id())?;
        if !map.flags().contains(MapFlags::BOOTH_ENABLED) {
            actor
                .send(MsgTalk::from_system(
                    character.id(),
                    TalkChannel::System,
                    "Player shops are not available on this map.",
                ))
                .await?;
            return Ok(());
        }
        // One booth a flag: someone may already sit where this one would.
        let here = character.entity().location();
        let taken = state.player_shop_manager().owners().into_iter().any(|owner| {
            owner != character.id()
                && state.entity(owner).is_some_and(|entity| {
                    entity.as_character().is_some_and(|seller| {
                        seller.entity().map_id() == character.entity().map_id()
                            && seller.entity().location().x == here.x
                            && seller.entity().location().y == here.y
                    })
                })
        });
        if taken {
            actor
                .send(MsgTalk::from_system(
                    character.id(),
                    TalkChannel::System,
                    "Someone is already selling at this flag.",
                ))
                .await?;
            return Ok(());
        }
        match state
            .player_shop_manager()
            .open(character.id(), character.entity().map_id())
        {
            Ok(_) => {
                let mut response = self.clone();
                response.data1 = character.id();
                actor.send(response.clone()).await?;
                actor.screen().send_message(response).await?;
                // The booth's name, and what it listed last time, still in the bag.
                let pool = state.pool();
                let name = crate::player_shop::saved_name(pool, character.database_id())
                    .await?
                    .unwrap_or_else(|| format!("{}'s Shop", character.entity().name()));
                character.set_booth_name(Some(name.clone()));
                let sign = MsgTalk::booth(character, &name);
                actor.send(sign.clone()).await?;
                actor.screen().send_message(sign).await?;
                let bag = character.inventory_items();
                let mut restored = 0;
                for listing in crate::player_shop::saved_listings(pool, character.database_id()).await? {
                    let item = bag.iter().find(|item| item.uid == listing.item_uid);
                    match item {
                        Some(item) if state.player_shop_manager().restore_item(character.id(), listing).is_ok() => {
                            actor
                                .send(super::MsgVendorItem::new(character.id(), *item, listing))
                                .await?;
                            restored += 1;
                        },
                        _ => crate::player_shop::forget_listing(pool, listing.item_uid).await?,
                    }
                }
                let line = if restored > 0 {
                    format!("{name} is open again with {restored} item(s) from last time.")
                } else {
                    format!("{name} is open. Add items to sell.")
                };
                actor
                    .send(MsgTalk::from_system(character.id(), TalkChannel::System, line))
                    .await?;
            },
            Err(error) => {
                actor
                    .send(MsgTalk::from_system(
                        character.id(),
                        TalkChannel::System,
                        error.message(),
                    ))
                    .await?;
            },
        }
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn handle_jump(&self, state: &State, actor: &Actor<ActorState>) -> Result<(), Error> {
        let new_x = self.data1.lo();
        let new_y = self.data1.hi();
        let current_x = self.data2.lo();
        let current_y = self.data2.hi();
        let entity = actor.try_entity()?;
        let me = entity.as_character().ok_or(Error::CharacterNotFound)?;
        // A seller stays at their booth until they leave it.
        if selling(state, actor, me).await? {
            return Ok(());
        }
        let loc = me.entity().location();
        let mymap_id = me.entity().map_id();
        // Starting to validate this jump.
        if current_x != loc.x || current_y != loc.y {
            tracing::debug!(%current_x, %current_y, %loc.x, %loc.y, "Bad Jump Packet");
            me.kick_back().await?;
            return Ok(());
        }

        if !tq_math::in_screen((loc.x, loc.y), (new_x, new_y)) {
            tracing::debug!(%loc.x, %loc.y, %new_x, %new_y, "Bad Location Distance > 18");
            me.kick_back().await?;
            return Ok(());
        }

        // Jumping faster than a client can: back to where the server has them.
        let cells = f32::from(loc.x.abs_diff(new_x)).hypot(f32::from(loc.y.abs_diff(new_y)));
        if !me.keeps_pace(crate::movement_pace::jump_cost(cells), "jump") {
            me.kick_back().await?;
            return Ok(());
        }

        let mymap = state.try_map(mymap_id)?;
        let within_elevation = mymap.sample_elevation((loc.x, loc.y), (new_x, new_y), me.elevation());
        if !within_elevation {
            tracing::debug!(%loc.x, %loc.y, %new_x, %new_y, "Elevation diff > 210");
            me.kick_back().await?;
            return Ok(());
        }

        let direction = walk_direction((loc.x, loc.y), (new_x, new_y));
        let gate_closed = crate::guild_war::path_blocked(state, mymap_id, (loc.x, loc.y), (new_x, new_y));
        match mymap.tile(new_x, new_y) {
            Some(tile) if tile.access > TileType::Npc && !gate_closed => {
                // I guess everything seems to be valid .. send the jump.
                me.entity()
                    .set_location(Location::new(new_x, new_y, direction))
                    .set_action(100);
                me.set_elevation(tile.elevation);
                mymap.update_region_for(entity.clone());
                // The client leaves the direction out; observers turn the
                // jumper by it.
                let mut jump = self.clone();
                jump.details = u16::from(direction);
                actor.send(jump.clone()).await?;
                let myscreen = actor.screen();
                myscreen.send_movement(state, jump).await?;
            },
            Some(_) | None => {
                // Invalid Location move them back
                let msg = MsgTalk::from_system(me.id(), TalkChannel::TopLeft, String::from("Invalid Location"));
                actor.send(msg).await?;
                me.kick_back().await?;
                tracing::debug!(id = %me.id(), %loc.x, %loc.y, %new_x, %new_y, "Invalid Location");
                return Ok(());
            },
        };
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn handle_change_facing(&self, actor: &Actor<ActorState>) -> Result<(), Error> {
        let current_x = self.data2.lo();
        let current_y = self.data2.hi();
        let entity = actor.try_entity()?;
        let me = entity.as_character().ok_or(Error::CharacterNotFound)?;
        let mut loc = me.entity().location();

        // Starting to validate this action.
        if current_x != loc.x || current_y != loc.y {
            // Kick Back.
            me.kick_back().await?;
            return Ok(());
        }

        loc.direction = self.details as u8;
        me.entity().set_location(loc);
        actor.send(self.clone()).await?;
        let myscreen = actor.screen();
        myscreen.send_message(self.clone()).await?;
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn handle_change_action(&self, actor: &Actor<ActorState>) -> Result<(), Error> {
        let entity = actor.try_entity()?;
        let me = entity.as_character().ok_or(Error::CharacterNotFound)?;
        if self.character_id != me.id() {
            me.kick_back().await?;
            return Ok(());
        }
        me.entity().set_action(self.data1 as u16);
        actor.send(self.clone()).await?;
        actor.screen().send_message(self.clone()).await?;
        Ok(())
    }

    /// MsgAction 106: where a teammate stands, for the minimap pin. The
    /// reply is about the member: data1 is the map (0 when it is not on the
    /// asker's map) and data2 the packed cell.
    async fn handle_query_team_member(&self, state: &State, actor: &Actor<ActorState>) -> Result<(), Error> {
        let entity = actor.try_entity()?;
        let me = entity.as_character().ok_or(Error::CharacterNotFound)?;
        if self.character_id != me.id() || !state.team_manager().same_team(me.id(), self.data1) {
            return Ok(());
        }
        let map_id = me.entity().map_id();
        let member = state.entity(self.data1);
        let reply = match member
            .as_ref()
            .and_then(|member| member.as_character())
            .filter(|member| member.entity().map_id() == map_id)
        {
            Some(member) => Self::from_character(member, map_id, ActionType::QueryTeamMember),
            None => Self::new(self.data1, 0, 0, 0, ActionType::QueryTeamMember),
        };
        actor.send(reply).await?;
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn handle_query_entity(&self, state: &State, actor: &Actor<ActorState>) -> Result<(), Error> {
        let entity = actor.try_entity()?;
        let me = entity.as_character().ok_or(Error::CharacterNotFound)?;
        let mymap_id = me.entity().map_id();
        let mymap = state.try_map(mymap_id)?;
        let other = mymap.with_regions(|r| r.iter().find_map(|r| r.try_entities(self.data1)));
        if let Some(other) = other.and_then(|o| o.upgrade()).as_ref().and_then(|o| o.as_character()) {
            let msg = super::MsgPlayer::from(other);
            actor.send(msg).await?;
        } else {
            let msg = super::MsgTalk::from_system(me.id(), TalkChannel::System, "Player not found");
            actor.send(msg).await?;
        }
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn handle_change_map(&self, state: &State, actor: &Actor<ActorState>) -> Result<(), Error> {
        let portal_x = self.data1.lo();
        let portal_y = self.data1.hi();
        let entity = actor.try_entity()?;
        let me = entity.as_character().ok_or(Error::CharacterNotFound)?;
        let loc = me.entity().location();
        let mymap_id = me.entity().map_id();
        if !tq_math::in_screen((loc.x, loc.y), (portal_x, portal_y)) {
            tracing::warn!(
                target: "world",
                character = %me.entity().name(),
                account = actor.id(),
                %portal_x,
                %portal_y,
                %loc.x,
                %loc.y,
                "Portal used from out of sight; sent back"
            );
            me.kick_back().await?;
            return Ok(());
        }
        let mymap = state.try_map(mymap_id)?;
        let maybe_portal = mymap
            .portals()
            .iter()
            .find(|p| tq_math::in_circle((loc.x, loc.y, 5), (p.from_x(), p.from_y())));
        match maybe_portal {
            Some(portal) => {
                let portal_map = state.try_map(portal.to_map_id())?;
                mymap.remove_entity(&entity)?;
                portal_map.insert_entity(entity.clone()).await?;
                me.teleport(state, portal.to_map_id(), (portal.to_x(), portal.to_y()))
                    .await?;
            },
            None => {
                tracing::debug!(%portal_x, %portal_y, %loc.x, %loc.y, "Portal not found");
                me.kick_back().await?;
            },
        }
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn handle_set_kill_mode(&self, _state: &State, actor: &Actor<ActorState>) -> Result<(), Error> {
        let kill_mode = match self.data1 as u16 {
            0 => KillMode::Free,
            1 => KillMode::Safe,
            2 => KillMode::Team,
            3 => KillMode::Arrestment,
            _ => {
                tracing::debug!(
                    character_id = self.character_id,
                    mode = self.data1,
                    "Rejected invalid kill mode"
                );
                return Ok(());
            },
        };
        let entity = actor.try_entity()?;
        let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
        if self.character_id != character.id() {
            tracing::warn!(
                packet_character_id = self.character_id,
                actor_character_id = character.id(),
                "Rejected kill mode update from another character"
            );
            return Ok(());
        }
        character.set_kill_mode(kill_mode as u16);
        let notice = match kill_mode {
            KillMode::Free => "In free mode, you can attack everybody.",
            KillMode::Safe => "In safe mode, you can only attack monsters.",
            KillMode::Team => {
                "In team mode, you can attack everybody, except your friends, your teammates, and your guildmates."
            },
            KillMode::Arrestment => "In arrestment mode, you can only attack monsters and black name players.",
        };
        actor.send(self.clone()).await?;
        let msg = super::MsgTalk::from_system(actor.entity().id(), TalkChannel::System, notice);
        actor.send(msg).await?;
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn handle_reborn(&self, state: &State, actor: &Actor<ActorState>) -> Result<(), Error> {
        let entity = actor.try_entity()?;
        let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
        if self.character_id != character.id() {
            tracing::warn!(
                packet_character_id = self.character_id,
                actor_character_id = character.id(),
                "Rejected reborn request from another character"
            );
            return Ok(());
        }
        if character.entity().is_alive() {
            tracing::debug!(
                character_id = character.id(),
                "Rejected reborn request from living character"
            );
            return Ok(());
        }

        // data 1: revive where the body lies (Heaven's Blessing only).
        if !crate::death::revive(state, character, self.data1 == 1).await? {
            actor
                .send(super::MsgTalk::from_system(
                    character.id(),
                    TalkChannel::TopLeft,
                    "You can`t revive at the moment!",
                ))
                .await?;
            return Ok(());
        }
        let reborn = MsgAction::from_character(character, 0, ActionType::Reborn);
        let player = MsgPlayer::from(character);
        actor.send(MsgUserInfo::from(character)).await?;
        actor.send(reborn.clone()).await?;
        actor.send(player.clone()).await?;
        actor.screen().send_message(reborn).await?;
        actor.screen().send_message(player).await?;
        Ok(())
    }
}

#[async_trait]
impl PacketProcess for MsgAction {
    type ActorState = ActorState;
    type Error = Error;
    type State = State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        let ty = self.action_type.into();
        match ty {
            ActionType::SendLocation => self.handle_send_location(state, actor).await,
            ActionType::MapARGB => self.handle_map_argb(state, actor).await,
            ActionType::SetKillMode => self.handle_set_kill_mode(state, actor).await,
            ActionType::LeaveBooth => self.handle_leave_booth(state, actor).await,
            ActionType::CreateBooth => self.handle_create_booth(state, actor).await,
            ActionType::SendItems => {
                let entity = actor.entity();
                let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
                for item in character.equipment_items() {
                    actor.send(super::MsgItemInfo::equipment_item(item)).await?;
                }
                for item in character.inventory_items() {
                    actor.send(super::MsgItemInfo::inventory_item(item)).await?;
                }
                Ok(())
            },
            ActionType::Reborn => self.handle_reborn(state, actor).await,
            ActionType::SendAssociates => {
                // Friends and the ignore list.
                let entity = actor.try_entity()?;
                let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
                super::msg_friend::send_friend_list(state, actor, character.id()).await?;
                super::msg_friend::send_ignore_list(state, actor, character.id()).await?;
                actor.send(self.clone()).await?;
                Ok(())
            },
            ActionType::SendProficiencies => {
                // Weapon proficiencies.
                let entity = actor.try_entity()?;
                let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
                for packet in super::MsgWeaponSkill::for_all(character.weapon_proficiencies()) {
                    actor.send(packet).await?;
                }
                actor.send(self.clone()).await?;
                Ok(())
            },
            ActionType::SendSpells => {
                // Magic spells.
                let entity = actor.try_entity()?;
                let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
                for skill in character.skills() {
                    actor.send(super::MsgSkill::from(skill)).await?;
                }
                actor.send(self.clone()).await?;
                Ok(())
            },
            ActionType::ConfirmGuild => {
                // The guild and its members.
                let entity = actor.try_entity()?;
                let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
                if let Some(guild) = state.guild_manager().guild_for(character.id()) {
                    actor
                        .send(super::MsgGuild::new(super::SET_GUILD, guild.id).with_strings([guild.name.clone()]))
                        .await?;
                    super::send_guild_members(state, character.id(), &guild).await?;
                }
                actor.send(self.clone()).await?;
                Ok(())
            },
            ActionType::LoginCompeleted => self.handle_login_completed(state, actor).await,
            ActionType::Jump => self.handle_jump(state, actor).await,
            ActionType::ChangeFacing => self.handle_change_facing(actor).await,
            ActionType::ChangeAction => self.handle_change_action(actor).await,
            ActionType::QueryTeamMember => self.handle_query_team_member(state, actor).await,
            ActionType::Mine => {
                let entity = actor.try_entity()?;
                let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
                if self.character_id == character.id() {
                    crate::mining::start(state, character).await?;
                }
                Ok(())
            },
            ActionType::QueryEntity => self.handle_query_entity(state, actor).await,
            // The client's CancelFly: the flier lands.
            ActionType::TakeOff => {
                let entity = actor.try_entity()?;
                let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
                if self.character_id == character.id() && character.has_status(crate::entities::StatusEffect::Flying) {
                    character.apply_status(crate::entities::StatusEffect::Flying, std::time::Duration::ZERO);
                    let update = super::MsgPlayer::from(character);
                    actor.send(update.clone()).await?;
                    actor.send(super::MsgUserInfo::from(character)).await?;
                    actor.screen().send_message(update).await?;
                }
                Ok(())
            },
            ActionType::ChangeMap => self.handle_change_map(state, actor).await,
            _ => {
                let p = MsgTalk::from_system(
                    self.character_id,
                    TalkChannel::Talk,
                    format!("Missing Action Type {ty:?} = {}", self.action_type),
                );
                actor.send(p).await?;
                let res = self.clone();
                actor.send(res).await?;
                tracing::warn!("Missing Action Type {ty:?} = {}", self.action_type);
                Ok(())
            },
        }
    }
}

/// The direction from `from` towards `to` in the MsgWalk convention
/// (`WALK_XCOORDS`/`WALK_YCOORDS`: 0 is +y, 2 is -x, 4 is -y, 6 is +x), so
/// observers and later MsgPlayer spawns face a jumper the way it jumped.
/// Mirrors the client's `direction_towards`.
fn walk_direction(from: (u16, u16), to: (u16, u16)) -> u8 {
    let dx = i32::from(to.0) - i32::from(from.0);
    let dy = i32::from(to.1) - i32::from(from.1);
    if dx == 0 && dy == 0 {
        return 3;
    }
    if dx == 0 {
        return if dy > 0 { 0 } else { 4 };
    }
    if dy == 0 {
        return if dx > 0 { 6 } else { 2 };
    }
    // tan(22.5°) ~ 0.41 and tan(67.5°) ~ 2.41 split the eight sectors.
    let signed_y = dy * 100 * dx.signum();
    let abs_x = dx.abs();
    let east = dx > 0;
    if (-241 * abs_x..-41 * abs_x).contains(&signed_y) {
        if east { 5 } else { 1 }
    } else if (-41 * abs_x..41 * abs_x).contains(&signed_y) {
        if east { 6 } else { 2 }
    } else if (41 * abs_x..241 * abs_x).contains(&signed_y) {
        if east { 7 } else { 3 }
    } else if dy > 0 {
        0
    } else {
        4
    }
}

#[cfg(test)]
mod tests {
    use super::walk_direction;
    use crate::constants::{WALK_XCOORDS, WALK_YCOORDS};

    #[test]
    fn jump_direction_matches_the_walk_direction() {
        for direction in 0..8u8 {
            let dx = i32::from(WALK_XCOORDS[usize::from(direction)]);
            let dy = i32::from(WALK_YCOORDS[usize::from(direction)]);
            for distance in [1, 5, 16] {
                let to = ((100 + dx * distance) as u16, (100 + dy * distance) as u16);
                assert_eq!(walk_direction((100, 100), to), direction, "{dx},{dy} x{distance}");
            }
        }
    }

    #[test]
    fn jump_direction_rounds_to_the_nearest_sector() {
        // Mostly +x with a little +y is still +x.
        assert_eq!(walk_direction((100, 100), (110, 102)), 6);
        // Mostly -y with a little +x is still -y.
        assert_eq!(walk_direction((100, 100), (102, 90)), 4);
    }
}

/// Takes `character`'s booth down: the listings end (the items never left
/// the bag), the sign over it goes, and the people around see them stand.
/// Whether `character` sells at an open booth, in which case a step or a
/// jump is refused: they stay put until they leave it.
pub(crate) async fn selling(
    state: &State,
    actor: &Actor<ActorState>,
    character: &crate::entities::Character,
) -> Result<bool, Error> {
    if state.player_shop_manager().view(character.id()).is_none() {
        return Ok(false);
    }
    character.kick_back().await?;
    actor
        .send(MsgTalk::from_system(
            character.id(),
            TalkChannel::System,
            "You are selling: leave your booth to go.",
        ))
        .await?;
    Ok(true)
}

pub(crate) async fn leave_booth(
    state: &State,
    actor: &Actor<ActorState>,
    character: &crate::entities::Character,
) -> Result<(), Error> {
    if state.player_shop_manager().close(character.id()).is_none() {
        return Ok(());
    }
    if character.booth_name().is_some() {
        character.set_booth_name(None);
        let sign = MsgTalk::booth(character, "");
        actor.send(sign.clone()).await?;
        actor.screen().send_message(sign).await?;
    }
    // Remove Player from Booth.
    let myscreen = actor.screen();
    myscreen.clear()?;
    myscreen.load_surroundings(state).await?;
    actor
        .send(MsgTalk::from_system(
            character.id(),
            TalkChannel::System,
            "Player shop closed.",
        ))
        .await?;
    Ok(())
}
