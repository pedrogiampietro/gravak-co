use crate::combat::next_roll;
use crate::Error;
use sqlx::PgPool;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonsterDrop {
    pub item_id: u32,
    pub chance_percent: u8,
    pub min_quantity: u16,
    pub max_quantity: u16,
    pub drop_group: u16,
    pub min_monster_level: u16,
    pub max_monster_level: u16,
}

#[derive(Debug, Default)]
pub struct DropCatalog {
    drops: HashMap<u32, Vec<MonsterDrop>>,
}

#[derive(Debug, sqlx::FromRow)]
struct MonsterDropRow {
    monster_type_id: i64,
    item_id: i64,
    chance_percent: i64,
    min_quantity: i64,
    max_quantity: i64,
    drop_group: i64,
    min_monster_level: i64,
    max_monster_level: i64,
}

impl DropCatalog {
    pub async fn load(pool: &PgPool) -> Result<Self, Error> {
        let rows = sqlx::query_as::<_, MonsterDropRow>(
            "SELECT monster_type_id, item_id, chance_percent, min_quantity, max_quantity,
                    drop_group, min_monster_level, max_monster_level
             FROM monster_drops
             ORDER BY monster_type_id, item_id",
        )
        .fetch_all(pool)
        .await?;
        let mut drops = HashMap::<u32, Vec<MonsterDrop>>::new();
        for row in rows {
            let monster_type_id = u32::try_from(row.monster_type_id)
                .map_err(|_| Error::Other("monster drop has an invalid monster type".to_owned()))?;
            let item_id =
                u32::try_from(row.item_id).map_err(|_| Error::Other("monster drop has an invalid item".to_owned()))?;
            let chance_percent = u8::try_from(row.chance_percent)
                .ok()
                .filter(|chance| *chance <= 100)
                .ok_or_else(|| Error::Other("monster drop chance must be between 0 and 100".to_owned()))?;
            let min_quantity = u16::try_from(row.min_quantity)
                .ok()
                .filter(|quantity| *quantity > 0)
                .ok_or_else(|| Error::Other("monster drop minimum quantity must be positive".to_owned()))?;
            let max_quantity = u16::try_from(row.max_quantity)
                .ok()
                .filter(|quantity| *quantity >= min_quantity)
                .ok_or_else(|| Error::Other("monster drop maximum quantity must be at least the minimum".to_owned()))?;
            let drop_group = u16::try_from(row.drop_group)
                .map_err(|_| Error::Other("monster drop group must be non-negative".to_owned()))?;
            let min_monster_level = u16::try_from(row.min_monster_level)
                .map_err(|_| Error::Other("monster drop minimum level must be non-negative".to_owned()))?;
            let max_monster_level = u16::try_from(row.max_monster_level)
                .ok()
                .filter(|level| *level >= min_monster_level)
                .ok_or_else(|| Error::Other("monster drop maximum level must be at least the minimum".to_owned()))?;
            drops.entry(monster_type_id).or_default().push(MonsterDrop {
                item_id,
                chance_percent,
                min_quantity,
                max_quantity,
                drop_group,
                min_monster_level,
                max_monster_level,
            });
        }
        Ok(Self { drops })
    }

    pub fn len(&self) -> usize {
        self.drops.values().map(Vec::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.drops.values().all(Vec::is_empty)
    }

    pub fn roll(&self, monster_type_id: u32, monster_level: u16) -> Vec<u32> {
        self.select(monster_type_id, monster_level, || next_roll() % 100, || next_roll())
    }

    fn select<F, Q>(&self, monster_type_id: u32, monster_level: u16, mut roll: F, mut quantity_roll: Q) -> Vec<u32>
    where
        F: FnMut() -> u32,
        Q: FnMut() -> u32,
    {
        let eligible = self
            .drops
            .get(&monster_type_id)
            .into_iter()
            .flatten()
            .filter(|drop| (drop.min_monster_level..=drop.max_monster_level).contains(&monster_level))
            .collect::<Vec<_>>();
        let mut selected = Vec::new();
        let mut groups = HashMap::<u16, Vec<&MonsterDrop>>::new();
        for drop in eligible {
            if drop.drop_group == 0 {
                if roll() < u32::from(drop.chance_percent) {
                    append_quantity(&mut selected, drop, &mut quantity_roll);
                }
            } else {
                groups.entry(drop.drop_group).or_default().push(drop);
            }
        }
        for candidates in groups.values() {
            let total_weight = candidates
                .iter()
                .map(|drop| u32::from(drop.chance_percent))
                .sum::<u32>()
                .min(100);
            if total_weight == 0 || roll() >= total_weight {
                continue;
            }
            let mut choice = quantity_roll() % total_weight;
            for drop in candidates {
                let weight = u32::from(drop.chance_percent);
                if choice < weight {
                    append_quantity(&mut selected, drop, &mut quantity_roll);
                    break;
                }
                choice -= weight;
            }
        }
        selected
    }
}

fn append_quantity<F>(selected: &mut Vec<u32>, drop: &MonsterDrop, quantity_roll: &mut F)
where
    F: FnMut() -> u32,
{
    let range = u32::from(drop.max_quantity - drop.min_quantity) + 1;
    let quantity = u32::from(drop.min_quantity) + quantity_roll() % range;
    selected.extend(std::iter::repeat_n(drop.item_id, quantity as usize));
}

#[cfg(test)]
mod tests {
    use super::{DropCatalog, MonsterDrop};
    use std::collections::HashMap;

    #[test]
    fn selects_items_by_monster_and_chance() {
        let mut drops = HashMap::new();
        drops.insert(
            1,
            vec![
                MonsterDrop {
                    item_id: 1088001,
                    chance_percent: 80,
                    min_quantity: 2,
                    max_quantity: 3,
                    drop_group: 0,
                    min_monster_level: 0,
                    max_monster_level: 255,
                },
                MonsterDrop {
                    item_id: 1088000,
                    chance_percent: 9,
                    min_quantity: 1,
                    max_quantity: 1,
                    drop_group: 0,
                    min_monster_level: 0,
                    max_monster_level: 255,
                },
            ],
        );
        let catalog = DropCatalog { drops };
        let mut rolls = [79, 9].into_iter();
        let mut quantities = [0, 0].into_iter();
        assert_eq!(
            catalog.select(1, 1, || rolls.next().unwrap(), || quantities.next().unwrap()),
            vec![1088001, 1088001]
        );
        assert!(catalog.select(99, 1, || 0, || 0).is_empty());
    }

    #[test]
    fn quantity_roll_stays_inside_configured_bounds() {
        let mut drops = HashMap::new();
        drops.insert(
            1,
            vec![MonsterDrop {
                item_id: 1088001,
                chance_percent: 100,
                min_quantity: 2,
                max_quantity: 4,
                drop_group: 0,
                min_monster_level: 0,
                max_monster_level: 255,
            }],
        );
        let catalog = DropCatalog { drops };
        assert_eq!(catalog.select(1, 1, || 0, || 0).len(), 2);
        assert_eq!(catalog.select(1, 1, || 0, || 2).len(), 4);
    }

    #[test]
    fn level_filter_and_group_select_one_reward() {
        let mut drops = HashMap::new();
        drops.insert(
            1,
            vec![
                MonsterDrop {
                    item_id: 100,
                    chance_percent: 100,
                    min_quantity: 1,
                    max_quantity: 1,
                    drop_group: 7,
                    min_monster_level: 1,
                    max_monster_level: 5,
                },
                MonsterDrop {
                    item_id: 200,
                    chance_percent: 100,
                    min_quantity: 1,
                    max_quantity: 1,
                    drop_group: 7,
                    min_monster_level: 6,
                    max_monster_level: 10,
                },
                MonsterDrop {
                    item_id: 300,
                    chance_percent: 100,
                    min_quantity: 1,
                    max_quantity: 1,
                    drop_group: 8,
                    min_monster_level: 1,
                    max_monster_level: 10,
                },
            ],
        );
        let catalog = DropCatalog { drops };
        let mut level_three = catalog.select(1, 3, || 0, || 0);
        level_three.sort_unstable();
        assert_eq!(level_three, vec![100, 300]);
        let mut level_eight = catalog.select(1, 8, || 0, || 0);
        level_eight.sort_unstable();
        assert_eq!(level_eight, vec![200, 300]);
    }

    #[test]
    fn empty_catalog_reports_empty() {
        assert!(DropCatalog::default().is_empty());
        assert_eq!(DropCatalog::default().len(), 0);
    }
}
