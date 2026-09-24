use std::collections::HashMap;
use std::fs;
use std::path::Path;

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

#[derive(Debug, Clone, Copy)]
pub struct ItemTemplate {
    pub price: u32,
    pub durability: u16,
    pub max_durability: u16,
}

impl ShopCatalog {
    /// Loads the plain-text Shop.dat used by the 5065 client.
    ///
    /// The server keeps the catalog optional while the project is still being
    /// bootstrapped. Set SHOP_DATA to the client file, for example:
    /// `R:\\5065\\ini\\Shop.dat`.
    pub fn from_environment() -> Self {
        let Some(path) = std::env::var_os("SHOP_DATA") else {
            tracing::debug!("SHOP_DATA is not configured; shop catalog is empty");
            return Self::default();
        };
        let path = Path::new(&path);
        match fs::read_to_string(path) {
            Ok(contents) => match Self::parse(&contents) {
                Ok(catalog) => {
                    let mut catalog = catalog;
                    if let Some(itemtype_path) = std::env::var_os("ITEMTYPE_DATA") {
                        match fs::read(&itemtype_path) {
                            Ok(mut bytes) => {
                                decrypt_itemtype(&mut bytes);
                                catalog.items = parse_item_templates(&bytes);
                                tracing::info!(items = catalog.items.len(), path = ?itemtype_path, "Loaded item prices");
                            }
                            Err(error) => tracing::error!(%error, path = ?itemtype_path, "Failed to load item prices"),
                        }
                    } else {
                        tracing::warn!("ITEMTYPE_DATA is not configured; purchases will be disabled");
                    }
                    tracing::info!(shops = catalog.offers.len(), path = ?path, "Loaded shop catalog");
                    catalog
                },
                Err(error) => {
                    tracing::error!(%error, path = ?path, "Failed to parse shop catalog");
                    Self::default()
                },
            },
            Err(error) => {
                tracing::warn!(%error, path = ?path, "Shop catalog unavailable");
                Self::default()
            },
        }
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
        self.items.get(&item_id).copied().filter(|item| item.price > 0)
    }

    pub fn sell_price(&self, item_id: u32, durability: u16, max_durability: u16) -> Option<u32> {
        let base_price = self.item(item_id)?.price;
        let price = if max_durability == 0 {
            u64::from(base_price) / 3
        } else {
            u64::from(base_price)
                .saturating_mul(u64::from(durability))
                / u64::from(max_durability)
                / 3
        };
        u32::try_from(price).ok().filter(|price| *price > 0)
    }
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
        if price > 0 {
            if let Ok(price) = u32::try_from(price) {
                items.insert(
                    item_id,
                    ItemTemplate {
                        price,
                        durability: fields.get(21).and_then(|value| value.parse().ok()).unwrap_or(100),
                        max_durability: fields.get(22).and_then(|value| value.parse().ok()).unwrap_or(100),
                    },
                );
            }
        }
    }
    items
}

#[cfg(test)]
mod tests {
    use super::ShopCatalog;

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
}
