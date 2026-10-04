/// This struct encapsulates the game character for a player. The player
/// controls the character as the protagonist of the Conquer Online storyline.
/// The character is the persona of the player who controls it. The persona can
/// be altered using different avatars, hairstyles, and body types. The player
/// also controls the character's professions and abilities.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct Character {
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub character_id: i32,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub account_id: i32,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub realm_id: i32,
    pub name: String,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub mesh: i32,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub avatar: i16,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub hair_style: i16,
    pub silver: i64,
    pub cps: i64,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub current_class: i16,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub previous_class: i16,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub rebirths: i16,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub level: i16,
    pub experience: i64,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub map_id: i32,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub x: i16,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub y: i16,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub virtue: i16,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub strength: i16,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub agility: i16,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub vitality: i16,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub spirit: i16,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub attribute_points: i16,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub health_points: i16,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub mana_points: i16,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub kill_points: i16,
    pub nobility_donation: i64,
    pub equipped_title: i64,
    pub double_experience_until: i64,
    pub heaven_blessing_until: i64,
    /// How many times the experience while `double_experience_until` lasts.
    pub experience_multiplier: i64,
    /// Lucky time left, in seconds.
    pub lucky_time: i64,
}

#[derive(Debug)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct Location {
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub map_id: i32,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub x: i16,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub y: i16,
}

#[cfg(feature = "sqlx")]
impl Character {
    pub async fn from_account(pool: &sqlx::PgPool, id: u32) -> Result<Option<Self>, crate::Error> {
        let maybe_character = sqlx::query_as::<_, Self>("SELECT * FROM characters WHERE account_id = $1;")
            .bind(i64::from(id))
            .fetch_optional(pool)
            .await?;
        Ok(maybe_character)
    }

    pub async fn name_taken(pool: &sqlx::PgPool, name: &str) -> Result<bool, crate::Error> {
        let taken: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM characters WHERE name = $1)")
            .bind(name)
            .fetch_one(pool)
            .await?;
        Ok(taken)
    }

    pub async fn by_id(pool: &sqlx::PgPool, id: i32) -> Result<Self, crate::Error> {
        let c = sqlx::query_as::<_, Self>("SELECT * FROM characters WHERE character_id = $1;")
            .bind(id)
            .fetch_one(pool)
            .await?;
        Ok(c)
    }

    pub async fn save(self, pool: &sqlx::PgPool) -> Result<i32, crate::Error> {
        let (id,) = sqlx::query_as::<_, (i64,)>(
            "
            INSERT INTO characters
                (
                    account_id, realm_id, name, mesh, avatar,
                    hair_style, silver, current_class,
                    map_id, x, y, virtue, strength, agility,
                    vitality, spirit, health_points, mana_points,
                    nobility_donation, equipped_title
                )
            VALUES 
                (
                    $1, $2, $3, $4, $5, $6,
                    $7, $8, $9, $10, $11, $12,
                    $13, $14, $15, $16, $17, $18, $19, $20
                )
            RETURNING character_id;
            ",
        )
        .bind(self.account_id)
        .bind(self.realm_id)
        .bind(self.name)
        .bind(self.mesh)
        .bind(self.avatar)
        .bind(self.hair_style)
        .bind(self.silver)
        .bind(self.current_class)
        .bind(self.map_id)
        .bind(self.x)
        .bind(self.y)
        .bind(self.virtue)
        .bind(self.strength)
        .bind(self.agility)
        .bind(self.vitality)
        .bind(self.spirit)
        .bind(self.health_points)
        .bind(self.mana_points)
        .bind(self.nobility_donation)
        .bind(self.equipped_title)
        .fetch_one(pool)
        .await?;
        i32::try_from(id).map_err(|_| crate::Error::Db(sqlx::Error::Decode("character id out of range".into())))
    }

    pub async fn update(self, pool: &sqlx::PgPool) -> Result<(), crate::Error> {
        sqlx::query(
            "
            UPDATE characters
            SET 
                name = $1,
                mesh = $2,
                avatar = $3,
                hair_style = $4,
                silver = $5,
                current_class = $6,
                map_id = $7,
                x = $8, y = $9, 
                virtue = $10,
                strength = $11, 
                agility = $12,
                vitality = $13,
                spirit = $14,
                health_points = $15,
                mana_points = $16,
                nobility_donation = $17,
                equipped_title = $18,
                level = $19,
                experience = $20,
                double_experience_until = $21,
                heaven_blessing_until = $22,
                experience_multiplier = $23,
                lucky_time = $24
            WHERE character_id = $25;
            ",
        )
        .bind(self.name)
        .bind(self.mesh)
        .bind(self.avatar)
        .bind(self.hair_style)
        .bind(self.silver)
        .bind(self.current_class)
        .bind(self.map_id)
        .bind(self.x)
        .bind(self.y)
        .bind(self.virtue)
        .bind(self.strength)
        .bind(self.agility)
        .bind(self.vitality)
        .bind(self.spirit)
        .bind(self.health_points)
        .bind(self.mana_points)
        .bind(self.nobility_donation)
        .bind(self.equipped_title)
        .bind(self.level)
        .bind(self.experience)
        .bind(self.double_experience_until)
        .bind(self.heaven_blessing_until)
        .bind(self.experience_multiplier)
        .bind(self.lucky_time)
        .bind(self.character_id)
        .execute(pool)
        .await?;
        Ok(())
    }
}
