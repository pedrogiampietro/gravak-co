use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, Default)]
pub struct ShopOffer {
    pub npc_id: u32,
    pub name: String,
    pub item_ids: Vec<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct ShopCatalog {
    offers: HashMap<u32, ShopOffer>,
    items: HashMap<u32, ItemTemplate>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ItemTemplate {
    pub required_profession: u8,
    pub required_weapon_skill: u8,
    pub level: u32,
    pub required_sex: u8,
    pub required_strength: u16,
    pub required_agility: u16,
    pub required_vitality: u16,
    pub required_spirit: u16,
    pub attack_min: u32,
    pub attack_max: u32,
    pub defense: u32,
    pub magic_attack: u32,
    pub magic_defense: u32,
    pub attack_range: u16,
    pub attack_speed: u16,
    pub health_add: u16,
    pub mana_add: u16,
    pub price: u32,
    pub durability: u16,
    pub max_durability: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemporaryEffect {
    DoubleExperience(Duration),
    /// Experience times the first number (the ExpPill and ExpAmrita).
    BoostedExperience(u8, Duration),
    HeavenBlessing(Duration),
}

impl TemporaryEffect {
    /// How many times the experience, for an experience boost.
    pub fn experience_multiplier(self) -> Option<u8> {
        match self {
            Self::DoubleExperience(_) => Some(2),
            Self::BoostedExperience(multiplier, _) => Some(multiplier),
            Self::HeavenBlessing(_) => None,
        }
    }
}

impl ShopCatalog {
    /// Loads the plain-text Shop.dat used by the 5065 client.
    ///
    /// The server keeps the catalog optional while the project is still being
    /// bootstrapped. Set SHOP_DATA to the client file, for example:
    /// `R:\\5065\\ini\\Shop.dat`.
    pub fn from_environment() -> Self {
        let mut catalog = Self::default();
        if let Some(path) = std::env::var_os("SHOP_DATA") {
            let path = Path::new(&path);
            match fs::read_to_string(path) {
                Ok(contents) => match Self::parse(&contents) {
                    Ok(parsed) => {
                        catalog.offers = parsed.offers;
                        tracing::info!(shops = catalog.offers.len(), path = ?path, "Loaded shop catalog");
                    },
                    Err(error) => tracing::error!(%error, path = ?path, "Failed to parse shop catalog"),
                },
                Err(error) => tracing::warn!(%error, path = ?path, "Shop catalog unavailable"),
            }
        } else {
            tracing::debug!("SHOP_DATA is not configured; shop offers are empty");
        }

        if let Some(itemtype_path) = std::env::var_os("ITEMTYPE_DATA") {
            match fs::read(&itemtype_path) {
                Ok(mut bytes) => {
                    decrypt_itemtype(&mut bytes);
                    catalog.items = parse_item_templates(&bytes);
                    tracing::info!(items = catalog.items.len(), path = ?itemtype_path, "Loaded item templates");
                },
                Err(error) => tracing::error!(%error, path = ?itemtype_path, "Failed to load item templates"),
            }
        } else {
            tracing::warn!("ITEMTYPE_DATA is not configured; item validation and stats are disabled");
        }
        catalog
    }

    fn parse(contents: &str) -> Result<Self, String> {
        let mut catalog = Self::default();
        let mut current: Option<ShopOffer> = None;

        let finish = |catalog: &mut Self, current: &mut Option<ShopOffer>| {
            if let Some(offer) = current.take() {
                if offer.npc_id != 0 {
                    catalog.offers.insert(offer.npc_id, offer);
                }
            }
        };

        for (line_number, raw_line) in contents.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                if line.starts_with("[Shop") {
                    finish(&mut catalog, &mut current);
                    current = Some(ShopOffer::default());
                }
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(format!("invalid Shop.dat line {}", line_number + 1));
            };
            let Some(offer) = current.as_mut() else {
                continue;
            };
            match key.trim() {
                "ID" => {
                    offer.npc_id = value
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid shop NPC ID at line {}", line_number + 1))?;
                },
                "Name" => offer.name = value.trim().to_owned(),
                key if key
                    .strip_prefix("Item")
                    .is_some_and(|index| !index.is_empty() && index.chars().all(|c| c.is_ascii_digit())) =>
                {
                    let item_id = value
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid shop item at line {}", line_number + 1))?;
                    offer.item_ids.push(item_id);
                },
                _ => {},
            }
        }
        finish(&mut catalog, &mut current);
        Ok(catalog)
    }

    pub fn offer(&self, npc_id: u32) -> Option<&ShopOffer> {
        self.offers.get(&npc_id)
    }

    pub fn item(&self, item_id: u32) -> Option<ItemTemplate> {
        self.template(item_id).filter(|item| item.price > 0)
    }

    pub fn template(&self, item_id: u32) -> Option<ItemTemplate> {
        self.items.get(&item_id).copied()
    }

    /// Resolves the equipment position from the classic item family. The
    /// server uses this as a second line of defense because the client-provided
    /// slot is input, not authority.
    pub fn equipment_slot_for_item(item_id: u32) -> Option<u8> {
        match item_id / 1_000 {
            110..=119 => Some(1),
            120..=129 => Some(2),
            130..=139 => Some(3),
            150..=151 => Some(6),
            152..=159 | 170..=179 => Some(7),
            160..=169 => Some(8),
            180..=189 => Some(9),
            410..=499 => Some(4),
            // Two-handed weapons: bows, glaives, poleaxes, spears...
            500..=599 => Some(4),
            900..=909 => Some(5),
            // Arrows, beside a bow.
            1_050 => Some(5),
            _ => None,
        }
    }

    pub fn is_two_handed(item_id: u32) -> bool {
        (500..=599).contains(&(item_id / 1_000))
    }

    pub fn is_bow(item_id: u32) -> bool {
        item_id / 1_000 == 500
    }

    pub fn is_arrow(item_id: u32) -> bool {
        item_id / 1_000 == 1_050
    }

    pub fn weapon_family(item_id: u32) -> Option<u32> {
        Self::equipment_slot_for_item(item_id)
            .filter(|slot| matches!(slot, 4 | 5))
            .filter(|_| !Self::is_arrow(item_id))
            .map(|_| item_id / 10)
    }

    pub fn health_recovery_for_item(item_id: u32) -> Option<u16> {
        match item_id {
            1_000_000 => Some(70),
            1_000_010 => Some(100),
            1_000_020 => Some(250),
            1_000_030 => Some(500),
            1_002_000 => Some(800),
            1_002_010 => Some(1_200),
            1_002_020 => Some(2_000),
            1_002_050 | 721_331 => Some(3_000),
            _ => None,
        }
    }

    pub fn mana_recovery_for_item(item_id: u32) -> Option<u16> {
        match item_id {
            1_001_000 => Some(70),
            1_001_010 => Some(200),
            1_001_020 => Some(450),
            1_001_030 => Some(1_000),
            1_001_040 => Some(2_000),
            1_002_030 => Some(3_000),
            1_002_040 | 721_333 => Some(4_500),
            _ => None,
        }
    }

    pub fn consumable_bundle_for_item(item_id: u32) -> Option<(u32, u8)> {
        match item_id {
            720_010 => Some((1_000_030, 3)),
            720_011 => Some((1_002_000, 3)),
            720_012 => Some((1_002_010, 3)),
            720_013 => Some((1_002_020, 3)),
            721_330 => Some((1_002_050, 3)),
            720_014 => Some((1_001_030, 3)),
            720_015 => Some((1_001_040, 3)),
            720_016 => Some((1_002_030, 3)),
            720_017 | 721_332 => Some((1_002_040, 3)),
            // MeteorScroll and DBScroll (MillionaireLee's packs).
            720_027 => Some((1_088_001, 10)),
            720_028 => Some((1_088_000, 10)),
            _ => None,
        }
    }

    pub fn temporary_effect_for_item(item_id: u32) -> Option<TemporaryEffect> {
        match item_id {
            722_700 => Some(TemporaryEffect::DoubleExperience(Duration::from_secs(10 * 60))),
            723_017 => Some(TemporaryEffect::DoubleExperience(Duration::from_secs(60 * 60))),
            // As the client describes them: 300% and 500% for two hours.
            720_393 => Some(TemporaryEffect::BoostedExperience(3, Duration::from_secs(2 * 60 * 60))),
            720_394 => Some(TemporaryEffect::BoostedExperience(5, Duration::from_secs(2 * 60 * 60))),
            1_200_000 => Some(TemporaryEffect::HeavenBlessing(Duration::from_secs(3 * 24 * 60 * 60))),
            1_200_001 => Some(TemporaryEffect::HeavenBlessing(Duration::from_secs(7 * 24 * 60 * 60))),
            1_200_002 => Some(TemporaryEffect::HeavenBlessing(Duration::from_secs(30 * 24 * 60 * 60))),
            _ => None,
        }
    }

    pub fn sell_price(&self, item_id: u32, durability: u16, max_durability: u16) -> Option<u32> {
        let base_price = self.item(item_id)?.price;
        let price = if max_durability == 0 {
            u64::from(base_price) / 3
        } else {
            u64::from(base_price).saturating_mul(u64::from(durability)) / u64::from(max_durability) / 3
        };
        u32::try_from(price).ok().filter(|price| *price > 0)
    }

    pub fn repair_price(&self, item_id: u32, durability: u16, max_durability: u16) -> Option<u32> {
        if max_durability == 0 || durability >= max_durability {
            return Some(0);
        }
        let base_price = u64::from(self.item(item_id)?.price.max(1));
        let missing = u64::from(max_durability - durability);
        let cost = base_price
            .saturating_mul(missing)
            .saturating_add(u64::from(max_durability - 1))
            / u64::from(max_durability)
            / 10;
        Some(u32::try_from(cost.max(1)).unwrap_or(u32::MAX))
    }
}

/// Two-handed weapons need the left hand free, except a bow with its
/// arrows; arrows go only beside a bow. `None` when the item fits.
pub fn hand_conflict(item_id: u32, position: u8, right: Option<u32>, left: Option<u32>) -> Option<&'static str> {
    let two_handed = ShopCatalog::is_two_handed(item_id);
    if ShopCatalog::is_arrow(item_id) {
        return (!right.is_some_and(ShopCatalog::is_bow)).then_some("Arrows can only be used with a bow.");
    }
    if two_handed && position == 4 {
        let left_ok = left.is_none_or(|left| ShopCatalog::is_bow(item_id) && ShopCatalog::is_arrow(left));
        return (!left_ok).then_some("Take off the left-hand item to hold a two-handed weapon.");
    }
    if position == 5 && right.is_some_and(ShopCatalog::is_two_handed) {
        return Some("Your two-handed weapon needs both hands.");
    }
    None
}

fn decrypt_itemtype(bytes: &mut [u8]) {
    let mut seed = 9527u32;
    let mut key = [0u8; 128];
    for value in &mut key {
        seed = seed.wrapping_mul(0x343fdu32).wrapping_add(0x269ec3u32);
        *value = ((seed >> 16) & 0x7fff) as u8;
    }
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = (*byte ^ key[index % key.len()]).rotate_right((index % 8) as u32);
    }
}

fn parse_item_templates(bytes: &[u8]) -> HashMap<u32, ItemTemplate> {
    let mut items = HashMap::new();
    let contents = String::from_utf8_lossy(bytes);
    for line in contents.lines().skip(1) {
        let fields = line.split_ascii_whitespace().collect::<Vec<_>>();
        if fields.len() < 13 {
            continue;
        }
        let (Ok(item_id), Ok(price)) = (fields[0].parse::<u32>(), fields[12].parse::<i64>()) else {
            continue;
        };
        if let Ok(price) = u32::try_from(price.max(0)) {
            items.insert(
                item_id,
                ItemTemplate {
                    required_profession: fields.get(2).and_then(|value| value.parse().ok()).unwrap_or_default(),
                    required_weapon_skill: fields.get(3).and_then(|value| value.parse().ok()).unwrap_or_default(),
                    level: fields.get(4).and_then(|value| value.parse().ok()).unwrap_or_default(),
                    required_sex: fields.get(5).and_then(|value| value.parse().ok()).unwrap_or_default(),
                    required_strength: fields.get(6).and_then(|value| value.parse().ok()).unwrap_or_default(),
                    required_agility: fields.get(7).and_then(|value| value.parse().ok()).unwrap_or_default(),
                    required_vitality: fields.get(8).and_then(|value| value.parse().ok()).unwrap_or_default(),
                    required_spirit: fields.get(9).and_then(|value| value.parse().ok()).unwrap_or_default(),
                    attack_max: fields.get(14).and_then(|value| value.parse().ok()).unwrap_or_default(),
                    attack_min: fields.get(15).and_then(|value| value.parse().ok()).unwrap_or_default(),
                    defense: fields.get(16).and_then(|value| value.parse().ok()).unwrap_or_default(),
                    magic_attack: fields.get(29).and_then(|value| value.parse().ok()).unwrap_or_default(),
                    magic_defense: fields.get(30).and_then(|value| value.parse().ok()).unwrap_or_default(),
                    attack_range: fields.get(31).and_then(|value| value.parse().ok()).unwrap_or_default(),
                    attack_speed: fields.get(32).and_then(|value| value.parse().ok()).unwrap_or_default(),
                    health_add: fields.get(19).and_then(|value| value.parse().ok()).unwrap_or_default(),
                    mana_add: fields.get(20).and_then(|value| value.parse().ok()).unwrap_or_default(),
                    price,
                    durability: fields.get(21).and_then(|value| value.parse().ok()).unwrap_or(100),
                    max_durability: fields.get(22).and_then(|value| value.parse().ok()).unwrap_or(100),
                },
            );
        }
    }
    items
}

#[cfg(test)]
mod tests {
    #[test]
    fn hands_follow_the_two_handed_rules() {
        use super::{hand_conflict, ShopCatalog};
        assert_eq!(ShopCatalog::equipment_slot_for_item(500_003), Some(4));
        assert_eq!(ShopCatalog::equipment_slot_for_item(1_050_000), Some(5));
        assert_eq!(ShopCatalog::weapon_family(1_050_000), None);
        // Bow with arrows is fine; arrows without a bow are not.
        assert_eq!(hand_conflict(500_003, 4, None, Some(1_050_000)), None);
        assert!(hand_conflict(1_050_000, 5, Some(410_001), None).is_some());
        assert_eq!(hand_conflict(1_050_000, 5, Some(500_003), None), None);
        // A glaive needs the left hand free; a shield cannot join it.
        assert!(hand_conflict(510_003, 4, None, Some(900_000)).is_some());
        assert!(hand_conflict(900_000, 5, Some(510_003), None).is_some());
        assert_eq!(hand_conflict(410_001, 4, None, Some(900_000)), None);
    }

    use super::{parse_item_templates, ItemTemplate, ShopCatalog, TemporaryEffect};
    use std::time::Duration;

    #[test]
    fn parses_shop_dat_offers() {
        let catalog = ShopCatalog::parse(
            "[Header]\nAmount=1\n[Shop0]\nID=3\nName=Drugstore\nItemAmount=2\nItem0=1000000\nItem1=1000010\n",
        )
        .unwrap();
        let offer = catalog.offer(3).unwrap();
        assert_eq!(offer.name, "Drugstore");
        assert_eq!(offer.item_ids, [1000000, 1000010]);
    }

    #[test]
    fn parses_item_combat_values() {
        let mut fields = vec!["0".to_owned(); 39];
        fields[0] = "410003".to_owned();
        fields[2] = "20".to_owned();
        fields[3] = "3".to_owned();
        fields[4] = "10".to_owned();
        fields[6] = "4".to_owned();
        fields[7] = "6".to_owned();
        fields[8] = "12".to_owned();
        fields[12] = "1000".to_owned();
        fields[14] = "20".to_owned();
        fields[15] = "12".to_owned();
        fields[16] = "8".to_owned();
        fields[29] = "11".to_owned();
        fields[30] = "22".to_owned();
        fields[31] = "3".to_owned();
        fields[32] = "900".to_owned();
        fields[19] = "25".to_owned();
        fields[20] = "40".to_owned();
        fields[21] = "50".to_owned();
        fields[22] = "60".to_owned();
        let contents = format!("header\n{}\n", fields.join(" "));
        let item = parse_item_templates(contents.as_bytes()).get(&410003).copied().unwrap();
        assert_eq!(item.level, 10);
        assert_eq!(item.required_profession, 20);
        assert_eq!(item.required_weapon_skill, 3);
        assert_eq!(item.required_strength, 4);
        assert_eq!(item.required_agility, 6);
        assert_eq!(item.required_vitality, 12);
        assert_eq!(item.attack_min, 12);
        assert_eq!(item.attack_max, 20);
        assert_eq!(item.defense, 8);
        assert_eq!(item.magic_attack, 11);
        assert_eq!(item.magic_defense, 22);
        assert_eq!(item.attack_range, 3);
        assert_eq!(item.attack_speed, 900);
        assert_eq!(item.health_add, 25);
        assert_eq!(item.mana_add, 40);
        assert_eq!(item.price, 1000);
        assert_eq!(item.durability, 50);
        assert_eq!(item.max_durability, 60);
    }

    #[test]
    fn resolves_equipment_families_without_trusting_client_slot() {
        assert_eq!(ShopCatalog::equipment_slot_for_item(111003), Some(1));
        assert_eq!(ShopCatalog::equipment_slot_for_item(150003), Some(6));
        assert_eq!(ShopCatalog::equipment_slot_for_item(410003), Some(4));
        assert_eq!(ShopCatalog::equipment_slot_for_item(900003), Some(5));
        assert_eq!(ShopCatalog::weapon_family(410003), Some(41000));
        assert_eq!(ShopCatalog::weapon_family(900003), Some(90000));
        assert_eq!(ShopCatalog::weapon_family(111003), None);
        assert_eq!(ShopCatalog::equipment_slot_for_item(1_090_000), None);
    }

    #[test]
    fn resolves_health_potions() {
        assert_eq!(ShopCatalog::health_recovery_for_item(1_000_000), Some(70));
        assert_eq!(ShopCatalog::health_recovery_for_item(1_000_030), Some(500));
        assert_eq!(ShopCatalog::health_recovery_for_item(1_002_050), Some(3_000));
        assert_eq!(ShopCatalog::health_recovery_for_item(720_010), None);
        assert_eq!(ShopCatalog::health_recovery_for_item(1_090_000), None);
    }

    #[test]
    fn resolves_mana_potions() {
        assert_eq!(ShopCatalog::mana_recovery_for_item(1_001_000), Some(70));
        assert_eq!(ShopCatalog::mana_recovery_for_item(1_001_030), Some(1_000));
        assert_eq!(ShopCatalog::mana_recovery_for_item(1_002_040), Some(4_500));
        assert_eq!(ShopCatalog::mana_recovery_for_item(720_010), None);
        assert_eq!(ShopCatalog::mana_recovery_for_item(1_000_000), None);
    }

    #[test]
    fn repair_price_scales_with_missing_durability() {
        let mut catalog = ShopCatalog::default();
        catalog.items.insert(
            410001,
            ItemTemplate {
                required_profession: 0,
                required_weapon_skill: 0,
                level: 1,
                required_sex: 0,
                required_strength: 0,
                required_agility: 0,
                required_vitality: 0,
                required_spirit: 0,
                attack_min: 1,
                attack_max: 2,
                defense: 0,
                magic_attack: 0,
                magic_defense: 0,
                attack_range: 2,
                attack_speed: 800,
                health_add: 0,
                mana_add: 0,
                price: 1_000,
                durability: 100,
                max_durability: 100,
            },
        );
        assert_eq!(catalog.repair_price(410001, 100, 100), Some(0));
        assert_eq!(catalog.repair_price(410001, 50, 100), Some(50));
        assert_eq!(catalog.repair_price(410001, 0, 100), Some(100));
    }

    #[test]
    fn resolves_consumable_bundles() {
        assert_eq!(ShopCatalog::consumable_bundle_for_item(720_010), Some((1_000_030, 3)));
        assert_eq!(ShopCatalog::consumable_bundle_for_item(720_013), Some((1_002_020, 3)));
        assert_eq!(ShopCatalog::consumable_bundle_for_item(720_017), Some((1_002_040, 3)));
        assert_eq!(ShopCatalog::consumable_bundle_for_item(410_003), None);
        assert_eq!(
            ShopCatalog::temporary_effect_for_item(723_017),
            Some(TemporaryEffect::DoubleExperience(Duration::from_secs(3600)))
        );
        assert_eq!(
            ShopCatalog::temporary_effect_for_item(1_200_000),
            Some(TemporaryEffect::HeavenBlessing(Duration::from_secs(3 * 24 * 60 * 60)))
        );
    }
}
