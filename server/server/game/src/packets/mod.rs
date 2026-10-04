mod msg_broadcast;
pub use msg_broadcast::MsgBroadcast;
mod msg_name;
pub use msg_name::MsgName;
mod msg_connect;
pub use msg_connect::MsgConnect;

mod msg_assign_attributes;
pub use msg_assign_attributes::MsgAssignAttributes;

mod msg_talk;
pub use msg_talk::{MsgTalk, TalkChannel, TalkStyle};

mod msg_user_info;
pub use msg_user_info::MsgUserInfo;

mod msg_nobility;
pub use msg_nobility::MsgNobility;

mod msg_nobility_ranking;
pub use msg_nobility_ranking::{send_nobility_ranking, MsgNobilityRanking};

mod msg_action;
pub use msg_action::{ActionType, MsgAction};

mod msg_item;
pub use msg_item::MsgItem;

mod msg_transfer;
pub use msg_transfer::{MsgTransfer, MsgTransferKey};

mod msg_register;
pub use msg_register::{BaseClass, BodyType, MsgRegister};

mod msg_walk;
pub use msg_walk::{MovementType, MsgWalk};

mod msg_player;
pub use msg_player::MsgPlayer;

mod msg_item_info;
pub use msg_item_info::*;

mod msg_data;
pub use msg_data::MsgData;

mod msg_weather;
pub use msg_weather::{MsgWeather, WeatherKind};

mod msg_map_info;
pub use msg_map_info::{MapFlags, MsgMapInfo};

mod msg_npc_info;
pub use msg_npc_info::MsgNpcInfo;

mod msg_npc;
pub use msg_npc::MsgNpc;

mod msg_task_dialog;
pub use msg_task_dialog::MsgTaskDialog;

mod msg_team;
pub use msg_team::{MsgTeam, MsgTeamMember};

mod msg_team_action;
pub use msg_team_action::MsgTeamAction;

mod msg_friend;
pub use msg_friend::MsgFriend;

mod msg_guild;
pub use msg_guild::{send_guild_members, MsgGuild, SET_GUILD};

mod msg_guild_member;
pub use msg_guild_member::MsgGuildMember;

mod msg_guild_warehouse;
pub use msg_guild_warehouse::MsgGuildWarehouse;

mod msg_trade;
pub use msg_trade::{expire_trades, left_mid_trade, returned_from_trade, MsgTrade};

mod msg_warehouse;
pub use msg_warehouse::MsgWarehouse;

mod msg_interact;
pub use msg_interact::{InteractAction, MsgInteract};
pub(crate) use msg_interact::reward_monster_defeat;

mod msg_skill_effect;
pub use msg_skill_effect::{MsgSkillEffect, SkillTarget};

mod msg_skill;
pub use msg_skill::MsgSkill;

mod msg_weapon_skill;
pub use msg_weapon_skill::MsgWeaponSkill;

mod msg_user_attrib;
pub use msg_user_attrib::{MsgUserAttrib, UserAttribute};

/// After a cast was accepted: uses the XP circle of an XP skill and the
/// skill's stamina, and tells the client.
pub async fn spend_cast_costs(
    actor: &tq_network::Actor<crate::ActorState>,
    character: &crate::entities::Character,
    skill: &crate::skills::SkillDefinition,
) -> Result<(), crate::Error> {
    if skill.xp && character.use_xp_circle() {
        let update = MsgPlayer::from(character);
        actor.send(update.clone()).await?;
        actor.screen().send_message(update).await?;
    }
    if skill.stamina_cost > 0 && character.spend_stamina(skill.stamina_cost) {
        actor
            .send(MsgUserAttrib::new(
                character.id(),
                UserAttribute::Stamina,
                u64::from(character.stamina()),
            ))
            .await?;
    }
    Ok(())
}

mod msg_ground_item;
pub use msg_ground_item::MsgGroundItem;

mod msg_vendor_item;
pub use msg_vendor_item::MsgVendorItem;

mod msg_socket_gem;
pub use msg_socket_gem::MsgSocketGem;

mod msg_refinery;
pub use msg_refinery::MsgRefinery;

mod msg_refinery_info;
pub use msg_refinery_info::MsgRefineryInfo;

mod msg_item_rarity;
pub use msg_item_rarity::MsgItemRarity;

mod msg_item_rarity_reroll;
pub use msg_item_rarity_reroll::MsgItemRarityReroll;

mod msg_bounty;
pub use msg_bounty::{add_kills as add_bounty_kills, send_board, MsgBounty, MsgBountyState};

mod msg_status_timers;
pub use msg_status_timers::MsgStatusTimers;
mod msg_store;
pub use msg_store::{expire_vip, load_vip, send_store, show_vip, MsgStore, MsgStoreProduct, MsgStoreState};
