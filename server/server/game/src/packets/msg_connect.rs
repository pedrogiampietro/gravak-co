use super::{MsgGuild, MsgNobility, MsgRefineryInfo, MsgTalk, MsgUserInfo};
use crate::entities::Character;
use crate::packets::MsgData;
use crate::systems::Screen;
use crate::{ActorState, Error, State};
use serde::{Deserialize, Serialize};
use tq_network::{Actor, IntoErrorPacket, PacketID, PacketProcess};
use tq_serde::String10;

/// Message containing a connection request to the game server. Contains the
/// player's access token from the Account server, and the patch and language
/// versions of the game client.
#[derive(Debug, Default, Serialize, Deserialize, PacketID)]
#[packet(id = 1052)]
#[allow(dead_code)]
pub struct MsgConnect {
    pub token: u64,
    pub build_version: u16,
    pub language: String10,
    pub file_contents: u32,
}

#[async_trait::async_trait]
impl PacketProcess for MsgConnect {
    type ActorState = ActorState;
    type Error = Error;
    type State = State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        let info = state
            .remove_login_token(self.token)
            .map_err(|_| MsgTalk::login_invalid().error_packet())?;
        actor.generate_keys(self.token).await?;
        actor.set_id(info.account_id as usize);
        let gm: Option<bool> = sqlx::query_scalar("SELECT gm FROM accounts WHERE account_id = $1")
            .bind(i64::from(info.account_id))
            .fetch_optional(state.pool())
            .await?;
        actor.set_gm(gm.unwrap_or(false));
        let maybe_character = tq_db::character::Character::from_account(state.pool(), info.account_id).await?;
        match maybe_character {
            Some(character) => {
                tracing::info!(
                    target: "login",
                    account = info.account_id,
                    map = character.map_id,
                    level = character.level,
                    "{} entered the world",
                    character.name
                );
                let me = Character::new(actor.handle(), character);
                me.load_skills(state.pool(), state.skill_catalog()).await?;
                me.load_weapon_proficiencies(state.pool()).await?;
                me.load_spouse(state.pool()).await?;
                crate::offline_tg::logged_in(me.id());
                me.load_inventory(state.pool()).await?;
                me.load_equipment(state.pool()).await?;
                me.load_refineries(state.pool()).await?;
                me.load_artifacts(state.pool()).await?;
                me.refresh_equipment_resources(state.shop_catalog());
                let character_id = me.id();
                let refinery_info = me
                    .equipment_items()
                    .into_iter()
                    .flat_map(|item| {
                        let refinery = me
                            .refinery_effect(item.uid)
                            .map(|effect| MsgRefineryInfo::refinery(item.uid, effect));
                        let artifact = me
                            .artifact_effect(item.uid)
                            .map(|effect| MsgRefineryInfo::artifact(item.uid, effect));
                        refinery.into_iter().chain(artifact)
                    })
                    .collect::<Vec<_>>();
                let mymap_id = me.entity().map_id();
                let mymap = state
                    .try_map(mymap_id)
                    .map_err(|_| MsgTalk::login_invalid().error_packet())?;
                mymap.load().await?;
                let saved_location = me.entity().location();
                if let Some(corrected_location) = mymap.nearest_walkable_position(saved_location, 24) {
                    tracing::warn!(
                        character_id,
                        map_id = mymap_id,
                        from_x = saved_location.x,
                        from_y = saved_location.y,
                        to_x = corrected_location.x,
                        to_y = corrected_location.y,
                        "correcting character spawn from blocked map cell"
                    );
                    me.entity().set_location(corrected_location);
                    if let Some(tile) = mymap.tile(corrected_location.x, corrected_location.y) {
                        me.set_elevation(tile.elevation);
                    }
                    me.save(state).await?;
                }
                let screen = Screen::new(actor.handle());
                let msg = MsgUserInfo::from(&me);
                let nobility = MsgNobility::from(&me);
                let skills = me.skills();
                let weapon_skills = super::MsgWeaponSkill::for_all(me.weapon_proficiencies());
                actor.update(me, screen);
                mymap.insert_entity(actor.entity()).await?;
                state.insert_entity(actor.entity());
                actor.send(MsgTalk::login_ok()).await?;
                actor.send(msg).await?;
                actor.send(nobility).await?;
                super::send_nobility_ranking(state, actor).await?;
                actor
                    .send(super::MsgUserAttrib::new(
                        actor.entity().id(),
                        super::UserAttribute::Stamina,
                        u64::from(crate::vitals::MAX_STAMINA),
                    ))
                    .await?;
                for skill in skills {
                    actor.send(super::MsgSkill::from(skill)).await?;
                }
                for weapon_skill in weapon_skills {
                    actor.send(weapon_skill).await?;
                }
                for info in refinery_info {
                    actor.send(info).await?;
                }
                super::msg_friend::send_friend_list(state, actor, character_id).await?;
                super::msg_friend::send_ignore_list(state, actor, character_id).await?;
                super::msg_talk::send_chat_history(state, actor, character_id).await?;
                if let Some(guild) = state.guild_manager().guild_for(character_id) {
                    actor
                        .send(MsgGuild::new(super::SET_GUILD, guild.id).with_strings([guild.name.clone()]))
                        .await?;
                    super::send_guild_members(state, character_id, &guild).await?;
                }
                if let Some(me) = actor.entity().as_character() {
                    super::send_board(state, me).await?;
                    super::load_vip(state, me).await?;
                }
                actor.send(MsgData::now()).await?;
            },
            None => {
                tracing::info!(target: "login", account = info.account_id, "New account, sent to character creation");
                state.store_creation_token(self.token as u32, info.account_id, info.realm_id)?;
                actor.send(MsgTalk::login_new_role()).await?;
            },
        };
        Ok(())
    }
}
