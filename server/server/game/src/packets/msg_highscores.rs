//! Highscores (Gravak packets, not in the 5065 client): `MsgHighscores`
//! asks for a page, `MsgHighscoreRow` carries one ranked character (of the
//! page or the podium) and `MsgHighscoreState` the page's place and the
//! asker's own rank.

use crate::highscores::{Category, Entry, Profession};
use crate::{ActorState, Error, State};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tq_network::{Actor, PacketID, PacketProcess};

/// `page` asking for the asker's own page.
pub const OWN_PAGE: u32 = u32::MAX;
/// A row of the page, and one of the podium's three.
pub const PAGE_ROW: u32 = 0;
pub const PODIUM_ROW: u32 = 1;

#[derive(Debug, Default, Clone, Serialize, Deserialize, PacketID)]
#[packet(id = 9140)]
pub struct MsgHighscores {
    pub category: u32,
    pub profession: u32,
    pub page: u32,
}

/// One ranked character, `index` of `total` (index 0 starts the rows over).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, PacketID)]
#[packet(id = 9141)]
pub struct MsgHighscoreRow {
    pub index: u32,
    pub total: u32,
    pub kind: u32,
    pub rank: u32,
    pub value: u64,
    pub level: u32,
    pub rebirths: u32,
    pub class: u32,
    pub mesh: u32,
    pub name: String,
}

/// The page shown, how many there are, the asker's rank (0: not ranked)
/// and when the ranking was worked out (seconds since 1970).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, PacketID)]
#[packet(id = 9142)]
pub struct MsgHighscoreState {
    pub category: u32,
    pub profession: u32,
    pub page: u32,
    pub pages: u32,
    pub own_rank: u32,
    pub own_value: u64,
    pub updated_at: u64,
    /// The asker's look, for their portrait.
    pub own_mesh: u32,
}

fn row(index: usize, total: usize, kind: u32, rank: usize, entry: &Entry) -> MsgHighscoreRow {
    MsgHighscoreRow {
        index: u32::try_from(index).unwrap_or_default(),
        total: u32::try_from(total).unwrap_or_default(),
        kind,
        rank: u32::try_from(rank).unwrap_or_default(),
        value: entry.value,
        level: entry.level,
        rebirths: entry.rebirths,
        class: entry.class,
        mesh: entry.mesh,
        name: entry.name.clone(),
    }
}

#[async_trait]
impl PacketProcess for MsgHighscores {
    type ActorState = ActorState;
    type Error = Error;
    type State = State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        let entity = actor.entity();
        let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
        let Some(category) = Category::from_id(self.category) else {
            return Ok(());
        };
        let profession = Profession::from_id(self.profession);
        let page = (self.page != OWN_PAGE).then_some(self.page as usize);
        let asker = u32::try_from(character.database_id()).unwrap_or_default();
        let page = state.highscores().page(state.pool(), category, profession, page, asker).await?;
        let total = page.rows.len() + page.podium.len();
        let rows = page
            .rows
            .iter()
            .map(|(rank, entry)| (PAGE_ROW, *rank, entry))
            .chain(page.podium.iter().map(|(rank, entry)| (PODIUM_ROW, *rank, entry)));
        for (index, (kind, rank, entry)) in rows.enumerate() {
            actor.send(row(index, total, kind, rank, entry)).await?;
        }
        let (own_rank, own_value) = page.own.unwrap_or((0, 0));
        actor
            .send(MsgHighscoreState {
                category: category as u32,
                profession: profession as u32,
                page: u32::try_from(page.page).unwrap_or_default(),
                pages: u32::try_from(page.pages).unwrap_or_default(),
                own_rank: u32::try_from(own_rank).unwrap_or_default(),
                own_value,
                updated_at: page.updated_at,
                own_mesh: character.entity().mesh() + u32::from(character.avatar()) * 10_000,
            })
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tq_network::{PacketDecode, PacketEncode};

    #[test]
    fn rows_survive_the_wire() {
        let row = MsgHighscoreRow {
            index: 3,
            total: 13,
            kind: PODIUM_ROW,
            rank: 2,
            value: 130,
            level: 130,
            rebirths: 2,
            class: 15,
            mesh: 671_003,
            name: "Ikzordep".into(),
        };
        let (_, bytes) = row.clone().encode().expect("encodes");
        assert_eq!(MsgHighscoreRow::decode(&bytes).expect("decodes"), row);
    }
}
