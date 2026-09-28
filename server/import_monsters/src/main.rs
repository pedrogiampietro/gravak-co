use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone)]
struct MonsterType {
    id: i32,
    name: String,
    look: i32,
    level: i32,
    life: i32,
    attack_max: i32,
    attack_min: i32,
    defense: i32,
    attack_speed: i32,
    respawn_seconds: i32,
}

#[derive(Debug, Clone)]
struct MonsterSpawn {
    id: i32,
    monster_type_id: i32,
    map_id: i32,
    x: i16,
    y: i16,
    amount: i32,
    radius: i32,
}

fn parse_monster_type(line: &str) -> Option<MonsterType> {
    // Format: INSERT INTO `monstertype` VALUES ('0001', 'Pheasant', '0104',
    // '0033', '0006', '0005', '0001', '0008', '0000', '0000', '0040', '0000',
    // '0000', '0000', '0500', '0500', '0001', '3', '0000', '0000', '099',
    // '099', '099', '099', '099', '099', '099', '1000000', '1001000', '100');
    let line = line.strip_prefix("INSERT INTO `monstertype` VALUES ")?;
    let line = line.strip_suffix(';')?;
    let line = line.trim_start_matches('(').trim_end_matches(')');

    let parts: Vec<&str> = line.split(',').map(|s| s.trim().trim_matches('\'')).collect();
    if parts.len() < 30 {
        return None;
    }

    let id = parts[0].parse::<i32>().ok()?;
    let name = parts[1].to_string();
    let look = parts[2].parse::<i32>().ok()?;
    let life = parts[3].parse::<i32>().ok()?;
    let attack_max = parts[4].parse::<i32>().ok()?;
    let attack_min = parts[5].parse::<i32>().ok()?;
    let level = parts[16].parse::<i32>().ok()?;
    let attack_speed = parts[14].parse::<i32>().ok()?;
    let defense = parts[9].parse::<i32>().ok()?;

    Some(MonsterType {
        id,
        name,
        look,
        level,
        life,
        attack_max,
        attack_min,
        defense,
        attack_speed,
        respawn_seconds: 10, // default
    })
}

fn parse_spawn(line: &str) -> Option<MonsterSpawn> {
    // Format: INSERT INTO `spawns` VALUES ('1', '1002', '377', '451', '392',
    // '466', '1', '5', '10', '10');
    let line = line.strip_prefix("INSERT INTO `spawns` VALUES ")?;
    let line = line.strip_suffix(';')?;
    let line = line.trim_start_matches('(').trim_end_matches(')');

    let parts: Vec<&str> = line.split(',').map(|s| s.trim().trim_matches('\'')).collect();
    if parts.len() < 10 {
        return None;
    }

    let id = parts[0].parse().ok()?;
    let map_id = parts[1].parse().ok()?;
    let x1: i32 = parts[2].parse().ok()?;
    let y1: i32 = parts[3].parse().ok()?;
    let x2: i32 = parts[4].parse().ok()?;
    let y2: i32 = parts[5].parse().ok()?;
    let monster_type_id = parts[6].parse().ok()?;
    let amount_per: i32 = parts[7].parse().ok()?;
    let amount_max: i32 = parts[8].parse().ok()?;
    let _frequency: i32 = parts[9].parse().ok()?;

    // Use center point of spawn area
    let x = ((x1 + x2) / 2) as i16;
    let y = ((y1 + y2) / 2) as i16;
    let radius = ((x2 - x1).max(y2 - y1) / 2) as i32;

    Some(MonsterSpawn {
        id,
        monster_type_id,
        map_id,
        x,
        y,
        amount: amount_max,
        radius: radius.max(1),
    })
}

fn main() -> anyhow::Result<()> {
    let content = fs::read_to_string("R:/redux_backup.sql")?;

    let mut monster_types = Vec::new();
    let mut spawns = Vec::new();

    for line in content.lines() {
        if line.starts_with("INSERT INTO `monstertype` VALUES") {
            if let Some(mt) = parse_monster_type(line) {
                monster_types.push(mt);
            }
        } else if line.starts_with("INSERT INTO `spawns` VALUES") {
            if let Some(spawn) = parse_spawn(line) {
                spawns.push(spawn);
            }
        }
    }

    println!("Parsed {} monster types", monster_types.len());
    println!("Parsed {} spawns", spawns.len());

    // Generate SQLite INSERT statements
    let mut sql_output = String::new();

    // Monster types
    sql_output.push_str("-- Monster Types\n");
    for mt in &monster_types {
        sql_output.push_str(&format!(
            "INSERT OR IGNORE INTO monster_types (id, name, look, level, life, attack_min, attack_max, defense, attack_speed, respawn_seconds) VALUES ({}, '{}', {}, {}, {}, {}, {}, {}, {}, {});\n",
            mt.id, mt.name, mt.look, mt.level, mt.life, mt.attack_min, mt.attack_max, mt.defense, mt.attack_speed, mt.respawn_seconds
        ));
    }

    sql_output.push_str("\n-- Monster Spawns\n");
    for spawn in &spawns {
        sql_output.push_str(&format!(
            "INSERT OR IGNORE INTO monster_spawns (id, monster_type_id, map_id, x, y, amount, radius) VALUES ({}, {}, {}, {}, {}, {}, {});\n",
            spawn.id, spawn.monster_type_id, spawn.map_id, spawn.x, spawn.y, spawn.amount, spawn.radius
        ));
    }

    fs::write("R:/gravak-co/server/monsters_spawns.sql", sql_output)?;
    println!("Generated SQL file: monsters_spawns.sql");

    Ok(())
}
