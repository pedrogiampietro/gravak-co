use crate::entities::Character;
use crate::nobility::{DonateOutcome, NobilityStatus};
use super::TalkChannel;
use crate::{ActorState, Error, State};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tq_network::{Actor, PacketID, PacketProcess};

/// Server-authoritative nobility state for the local character, and the
/// client's donation.
///
/// `action = 3` is the status the server sends; `action = 1` is a donation
/// of `donation` silver the client asks for. A separate packet keeps the
/// donation amount out of the legacy UserInfo payload, so older clients can
/// still decode UserInfo safely.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PacketID)]
#[packet(id = 2064)]
pub struct MsgNobility {
    pub action: u32,
    pub character_id: u32,
    pub donation: u64,
    pub rank: u32,
    pub remaining: u64,
    pub title_id: u32,
    /// The place among the donors (0: none).
    pub position: u32,
    /// The Top titles the hero holds (`top_classes`).
    pub top_flags: u32,
}

impl MsgNobility {
    pub const DONATE: u32 = 1;
    /// The same donation paid in CPs (one per 50,000 silver).
    pub const DONATE_CPS: u32 = 2;
    /// The client asks for its status (what came at login is lost while
    /// its world loads).
    pub const QUERY: u32 = 4;
    pub const STATUS: u32 = 3;

    pub fn status(character_id: u32, status: NobilityStatus, title_id: u32) -> Self {
        Self {
            action: Self::STATUS,
            character_id,
            donation: status.donation,
            rank: status.rank as u32,
            remaining: status.remaining,
            title_id,
            position: status.position.unwrap_or(0),
            top_flags: 0,
        }
    }
}

impl From<&Character> for MsgNobility {
    fn from(character: &Character) -> Self {
        MsgNobility {
            top_flags: crate::top_classes::flags_of(character.database_id()),
            ..Self::status(character.id(), character.nobility_status(), character.equipped_title())
        }
    }
}

#[async_trait]
impl PacketProcess for MsgNobility {
    type ActorState = ActorState;
    type Error = Error;
    type State = State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        let entity = actor.entity();
        let me = entity.as_character().ok_or(Error::CharacterNotFound)?;
        if self.action == Self::QUERY {
            actor.send(MsgNobility::from(me)).await?;
            return Ok(());
        }
        if self.action != Self::DONATE && self.action != Self::DONATE_CPS {
            return Ok(());
        }
        let amount = u32::try_from(self.donation).unwrap_or(u32::MAX);
        donate(state, actor, me, amount, self.action == Self::DONATE_CPS).await
    }
}

/// Exchanges `amount` silver for nobility (the Nobility window's Donate and
/// the `$nobility-donate` command), telling the donor how it went and
/// everyone whose rank it changed their new standing.
pub async fn donate(
    state: &State,
    actor: &Actor<ActorState>,
    me: &Character,
    amount: u32,
    in_cps: bool,
) -> Result<(), Error> {
    let line = |text: String| super::MsgTalk::from_system(me.id(), TalkChannel::System, text);
    match me.donate_nobility_with(state.pool(), amount, in_cps).await? {
        DonateOutcome::Donated(status, changed) => {
            actor.send(super::MsgUserInfo::from(me)).await?;
            actor.send(MsgNobility::from(me)).await?;
            actor
                .send(line(format!(
                    "You donated {amount} silver{}. You are now {} ({} in all).",
                    if in_cps {
                        format!(" in CPs ({})", u64::from(amount).div_ceil(crate::nobility::SILVER_PER_CP))
                    } else {
                        String::new()
                    },
                    status.rank.label(),
                    status.donation
                )))
                .await?;
            // Those who see the donor see the new rank over their head.
            if let Ok(screen) = me.try_screen() {
                let _ = screen.send_message(super::MsgPlayer::from(me)).await;
            }
            announce(state, me.database_id(), &changed).await;
            super::send_nobility_ranking(state, actor).await?;
        },
        DonateOutcome::InvalidLevel => {
            actor
                .send(line(format!("Nobility requires level {}.", crate::nobility::MIN_LEVEL)))
                .await?;
        },
        DonateOutcome::InvalidAmount => {
            actor
                .send(line(format!(
                    "Each donation should be at least {} silver.",
                    crate::nobility::MIN_DONATION
                )))
                .await?;
        },
        DonateOutcome::InsufficientSilver => {
            let what = if in_cps { "CPs" } else { "silver" };
            actor.send(line(format!("You do not have that much {what}."))).await?;
        },
    }
    Ok(())
}

/// Tells the others online whose rank a donation changed (a seat lost or
/// won) their new standing.
async fn announce(state: &State, donor: i32, changed: &[(i32, crate::nobility::NobilityRank)]) {
    for entity in state.entities() {
        let Some(character) = entity.as_character() else {
            continue;
        };
        let id = character.database_id();
        if id == donor {
            continue;
        }
        if let Some((_, rank)) = changed.iter().find(|(changed_id, _)| *changed_id == id) {
            let _ = character.owner().send(MsgNobility::from(character)).await;
            if let Ok(screen) = character.try_screen() {
                let _ = screen.send_message(super::MsgPlayer::from(character)).await;
            }
            let text = format!("Your nobility is now {}.", rank.label());
            let _ = character
                .owner()
                .send(super::MsgTalk::from_system(character.id(), TalkChannel::System, text))
                .await;
        }
    }
}
