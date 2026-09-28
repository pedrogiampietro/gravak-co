use std::fs;

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
struct NpcCsv {
    uniq_id: i32,
    name: String,
    #[serde(rename = "Type")]
    npc_type: i32,
    look: i32,
    map_id: i32,
    x: i16,
    y: i16,
    base: i32,
    sort: i32,
    level: i32,
    life: i32,
    defence: i32,
    magic_def: i32,
}

fn main() -> anyhow::Result<()> {
    let content = fs::read_to_string("R:/gravak-co/server/data/NPCs.csv")?;

    let mut rdr = csv::Reader::from_reader(content.as_bytes());
    let mut npcs = Vec::new();

    for result in rdr.deserialize() {
        let npc: NpcCsv = result?;
        npcs.push(npc);
    }

    println!("Parsed {} NPCs", npcs.len());

    let mut sql_output = String::new();
    sql_output.push_str("-- NPCs\n");

    for npc in &npcs {
        sql_output.push_str(&format!(
            "INSERT OR IGNORE INTO npcs (id, name, type, look, map_id, x, y, base, sort, level, life, defence, magic_def) VALUES ({}, '{}', {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {});\n",
            npc.uniq_id, npc.name, npc.npc_type, npc.look, npc.map_id, npc.x, npc.y, npc.base, npc.sort, npc.level, npc.life, npc.defence, npc.magic_def
        ));
    }

    fs::write("R:/gravak-co/server/npcs.sql", sql_output)?;
    println!("Generated SQL file: npcs.sql");

    Ok(())
}
