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
