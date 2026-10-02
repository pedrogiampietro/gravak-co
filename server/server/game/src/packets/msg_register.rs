use super::MsgTalk;
use crate::entities::Character;
use crate::systems::Screen;
use crate::{ActorState, Error, State};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use tq_network::{Actor, IntoErrorPacket, PacketID, PacketProcess};
use tq_serde::{String16, TQMaskedPassword};

#[derive(Debug, Default, Serialize, Deserialize, PacketID)]
#[packet(id = 1001)]
pub struct MsgRegister {
    pub username: String16,
    pub character_name: String16,
    pub password: TQMaskedPassword,
    pub mesh: u16,
    pub class: u16,
    pub token: u32,
    /// Gravak: the Tinter dye (3..=9) of the starter coat; anything else
    /// leaves it plain.
    pub coat_dye: u32,
}

/// Every new character starts in the plain coat (any profession, any body,
/// level 1), worn, in the dye chosen on the creation screen.
pub const STARTER_COAT: u32 = 132_003;
/// The armor slot of the equipment.
const ARMOR_SLOT: i64 = 3;

/// Puts the starter coat on `character_id`, dyed `dye` when it is a Tinter
/// colour (3..=9).
pub async fn give_starter_coat(
    pool: &sqlx::SqlitePool,
    catalog: &crate::shop::ShopCatalog,
    character_id: i64,
    dye: u32,
) -> Result<(), Error> {
    let (durability, max_durability) = catalog
        .template(STARTER_COAT)
        .map_or((1198, 1198), |coat| (coat.durability, coat.max_durability));
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO equipment_items (character_id, item_type, position, durability, max_durability) \
         VALUES (?, ?, ?, ?, ?) RETURNING item_uid",
    )
    .bind(character_id)
    .bind(i64::from(STARTER_COAT))
    .bind(ARMOR_SLOT)
    .bind(i64::from(durability))
    .bind(i64::from(max_durability))
    .fetch_one(pool)
    .await?;
    if (3..=9).contains(&dye) {
        sqlx::query("INSERT INTO item_colors (item_uid, color) VALUES (?, ?)")
            .bind(uid)
            .bind(i64::from(dye))
            .execute(pool)
            .await?;
    }
    Ok(())
}

impl MsgRegister {
    pub fn build_character(&self, account_id: u32, realm_id: u32) -> Result<tq_db::character::Character, Error> {
        Self::build_character_with(
            self.character_name.to_string(),
            BodyType::try_from(self.mesh).map_err(|_| Error::InvalidBodyType)?,
            BaseClass::try_from(self.class).map_err(|_| Error::InvalidClass)?,
            account_id,
            realm_id,
        )
    }

    pub fn build_character_with(
        name: String,
        mesh: BodyType,
        class: BaseClass,
        account_id: u32,
        realm_id: u32,
    ) -> Result<tq_db::character::Character, Error> {
        // Some Math for rand characher.
        let mut rng = rand::rngs::StdRng::from_entropy();

        let avatar = match u16::from(mesh) {
            // For Male
            m if m < 1005 => rng.gen_range(1..49),

            // For Females
            _ => rng.gen_range(201..249),
        };

        let hair_style = rng.gen_range(3..9) * 100 + crate::constants::HAIR_STYLES[rng.gen_range(0..12)];
        // Redux: a new character starts with its class's level 1 attributes.
        let (strength, vitality, agility, spirit) =
            crate::stats::for_level(u16::from(class) as u8, 1).map_or((4, 12, 6, 0), |(s, v, a, sp)| {
                (i16::try_from(s).unwrap_or(0), i16::try_from(v).unwrap_or(0), i16::try_from(a).unwrap_or(0), i16::try_from(sp).unwrap_or(0))
            });

        // Start with full life and mana, from the same formulas the game
        // uses for their capacities.
        let attribute = |value: i16| u16::try_from(value).unwrap_or(0);
        let health = crate::entities::health_from_attributes(
            attribute(strength),
            attribute(agility),
            attribute(vitality),
            attribute(spirit),
            u16::from(class),
        );
        let mana = crate::entities::max_mana_for(attribute(spirit), u16::from(class));
        let health_points = i16::try_from(health.max(1)).unwrap_or(i16::MAX);
        let mana_points = i16::try_from(mana).unwrap_or(i16::MAX);

        let c = tq_db::character::Character {
            account_id: account_id as i32,
            realm_id: realm_id as i32,
            name,
            mesh: u16::from(mesh) as i32,
            avatar,
            hair_style,
            silver: 1000,
            cps: 0,
            current_class: u16::from(class) as i16,
            map_id: 1010,
            x: 61,
            y: 109,
            virtue: 0,
            strength,
            agility,
            vitality,
            spirit,
            health_points,
            mana_points,
            ..Default::default()
        };
        Ok(c)
    }
}

#[derive(Copy, Clone, Debug, TryFromPrimitive, IntoPrimitive)]
#[repr(u16)]
pub enum BodyType {
    AgileMale = 1003,
    MuscularMale = 1004,
    AgileFemale = 2001,
    MuscularFemale = 2002,
}

#[derive(Copy, Clone, Debug, TryFromPrimitive, IntoPrimitive)]
#[repr(u16)]
pub enum BaseClass {
    Trojan = 10,
    Warrior = 20,
    Archer = 40,
    Taoist = 100,
}

/// Character names are 3-15 ASCII letters or digits.
pub fn valid_character_name(name: &str) -> bool {
    (3..=15).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric())
}

#[async_trait::async_trait]
impl PacketProcess for MsgRegister {
    type ActorState = ActorState;
    type Error = Error;
    type State = State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        // Validate before spending the creation token, so the client can
        // retry with another name on the same connection.
        if !valid_character_name(&self.character_name) {
            return Err(MsgTalk::register_invalid_name().error_packet().into());
        }
        BodyType::try_from(self.mesh).map_err(|_| MsgTalk::register_invalid().error_packet())?;
        BaseClass::try_from(self.class).map_err(|_| MsgTalk::register_invalid().error_packet())?;
        if tq_db::character::Character::name_taken(state.pool(), &self.character_name).await? {
            return Err(MsgTalk::register_name_taken().error_packet().into());
        }

        let info = state
            .remove_creation_token(self.token)
            .map_err(|_| MsgTalk::register_invalid().error_packet())?;

        let character_id = self
            .build_character(info.account_id, info.realm_id)?
            .save(state.pool())
            .await?;
        give_starter_coat(state.pool(), state.shop_catalog(), i64::from(character_id), self.coat_dye).await?;
        let character = tq_db::character::Character::by_id(state.pool(), character_id).await?;
        let map_id = character.map_id;
        let me = Character::new(actor.handle(), character);
        me.load_equipment(state.pool()).await?;
        me.load_skills(state.pool(), state.skill_catalog()).await?;
        me.load_weapon_proficiencies(state.pool()).await?;
        let skills = me.skills();
        let screen = Screen::new(actor.handle());
        actor.update(me, screen);
        state.insert_entity(actor.entity());
        // Set player map.
        state
            .try_map(map_id as _)
            .map_err(|_| MsgTalk::register_invalid().error_packet())?
            .insert_entity(actor.entity())
            .await?;

        tracing::info!(
            "Account #{} Created Character #{} with Name {}",
            info.account_id,
            character_id,
            self.character_name
        );
        actor.send(MsgTalk::register_ok()).await?;
        for skill in skills {
            actor.send(super::MsgSkill::from(skill)).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn new_characters_wear_the_starter_coat_in_their_dye() -> Result<(), Error> {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(2)
            .connect("sqlite::memory:")
            .await?;
        sqlx::migrate!("../../migrations").run(&pool).await.expect("migrations apply");
        let catalog = crate::shop::ShopCatalog::default();
        for (case, dye, colour) in [(1, 6, Some(6)), (2, 0, None), (3, 12, None)] {
            let account: i64 = sqlx::query_scalar("INSERT INTO accounts (username, password) VALUES (?, '') RETURNING account_id")
                .bind(format!("coat{case}"))
                .fetch_one(&pool)
                .await?;
            let account = u32::try_from(account).expect("account id");
            let id = MsgRegister::build_character_with(format!("coat{case}"), BodyType::AgileFemale, BaseClass::Archer, account, 1)?
                .save(&pool)
                .await?;
            give_starter_coat(&pool, &catalog, i64::from(id), dye).await?;
            let (uid, item_type, position): (i64, i64, i64) = sqlx::query_as(
                "SELECT item_uid, item_type, position FROM equipment_items WHERE character_id = ?",
            )
            .bind(i64::from(id))
            .fetch_one(&pool)
            .await?;
            assert_eq!((item_type, position), (i64::from(STARTER_COAT), ARMOR_SLOT));
            let stored: Option<i64> = sqlx::query_scalar("SELECT color FROM item_colors WHERE item_uid = ?")
                .bind(uid)
                .fetch_optional(&pool)
                .await?;
            assert_eq!(stored, colour, "dye {dye}");
        }
        Ok(())
    }

    #[test]
    fn new_characters_keep_their_class_and_start_with_full_life() {
        for class in [BaseClass::Trojan, BaseClass::Warrior, BaseClass::Archer, BaseClass::Taoist] {
            let character =
                MsgRegister::build_character_with("Hero".into(), BodyType::AgileFemale, class, 1, 1).unwrap();
            assert_eq!(character.current_class, u16::from(class) as i16);
            assert!(character.strength + character.agility + character.vitality + character.spirit > 0);
            let health = crate::entities::health_from_attributes(
                character.strength as u16,
                character.agility as u16,
                character.vitality as u16,
                character.spirit as u16,
                u16::from(class),
            );
            assert_eq!(character.health_points, health as i16);
        }
    }

    #[test]
    fn character_names_are_letters_and_digits() {
        assert!(valid_character_name("Gravak01"));
        assert!(!valid_character_name("ab"));
        assert!(!valid_character_name("bad name"));
        assert!(!valid_character_name("Pedro[GM]"));
    }
}
