use bytes::{Buf, Bytes};
use std::collections::HashMap;
use std::io::{BufReader, Read};
use std::path::PathBuf;
use std::{env, fs};

#[derive(Debug, serde::Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct MapCsv {
    // Columns of the CSV the decoder reads but does not need.
    #[allow(dead_code)]
    uid: u32,
    #[allow(dead_code)]
    name: String,
    id: u32,
    flags: u32,
    weather: u8,
    portal_x: u16,
    portal_y: u16,
    reborn_map: u32,
    color: u32,
}

#[derive(Debug, serde::Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct Portal {
    id: u32,
    from_map_id: u32,
    from_x: u16,
    from_y: u16,
    to_map_id: u32,
    to_x: u16,
    to_y: u16,
}

#[derive(Debug, Clone)]
struct MapEntry {
    map_id: u32,
    path: String,
    #[allow(dead_code)]
    name: String,
    flags: u32,
    weather: u8,
    portal_x: u16,
    portal_y: u16,
    reborn_map: u32,
    color: u32,
}

fn default_metadata(map_id: u32, name: &str) -> (u32, u8, u16, u16, u32, u32) {
    let (flags, weather, portal_x, portal_y, reborn_map, color) = match map_id {
        1000 => (8192, 0, 499, 650, 1000, 4294967295),    // Desert
        1001 => (8192, 0, 0, 0, 1000, 4294967295),        // AncientCity
        1002 => (24576, 0, 430, 380, 1002, 4294967295),   // Newplain
        1003 => (8192, 0, 0, 0, 1000, 4294967295),        // Mine01
        1004 => (8200, 0, 0, 0, 1002, 4294967295),        // Forum
        1005 => (1, 0, 51, 50, 1005, 4294967295),         // Arena
        1006 => (8200, 0, 37, 31, 1002, 4294967295),      // Horse
        1007 => (8192, 0, 25, 24, 1002, 4294967295),      // Smith
        1008 => (8206, 0, 22, 25, 1002, 4294967295),      // Grocery
        1010 => (0, 0, 111, 98, 1010, 4294967295),        // Newbie
        1011 => (24576, 0, 193, 266, 1011, 4294967295),   // Woods
        1012 => (8192, 0, 25, 24, 1020, 4294967295),      // Sky
        1013 => (8192, 0, 22, 25, 1011, 4294967295),      // Tiger
        1014 => (8192, 0, 22, 25, 1011, 4294967295),      // Dragon
        1015 => (8192, 2, 717, 577, 1015, 4294967295),    // Island
        1016 => (8192, 0, 22, 25, 1011, 4294967295),      // Qiling
        1020 => (24576, 3, 566, 565, 1020, 4294967295),   // Canyon
        1023 => (10242, 10, 0, 0, 1000, 4294967295),      // CrystalMine-1
        1024 => (10242, 10, 0, 0, 1000, 4294967295),      // CrystalMine-2
        1025 => (10242, 10, 0, 0, 1011, 4294967295),      // CopperMine
        1026 => (10242, 10, 0, 0, 1020, 4294967295),      // SilverMine
        1027 => (10242, 10, 0, 0, 1000, 4294967295),      // GoldMine
        1028 => (10242, 10, 0, 0, 1002, 4294967295),      // MineCave
        1029 => (10242, 10, 0, 0, 1011, 4294967295),      // MineOne2
        1030 => (10242, 10, 0, 0, 1020, 4294967295),      // MineTwo2
        1031 => (10242, 10, 0, 0, 1000, 4294967295),      // MineThree2
        1032 => (10242, 10, 0, 0, 1002, 4294967295),      // MineFour2
        1036 => (30, 3, 303, 237, 1036, 4294967295),      // Street
        1038 => (130, 0, 0, 0, 1002, 4294967295),         // Faction
        1039 => (13, 0, 29, 72, 1002, 4294967295),        // Playground
        1042 => (8192, 0, 60, 108, 1002, 4294967295),     // LineupPass
        1043 => (8258, 0, 60, 108, 1002, 4294967295),     // Lineup
        1051 => (8192, 0, 0, 0, 1015, 4294967295),        // Riskisland
        1052 => (8192, 0, 724, 573, 1015, 4294967295),    // IdlandMap
        1060 => (8192, 0, 0, 0, 1002, 4294967295),        // Skymaze1
        1061 => (8192, 0, 0, 0, 1002, 4294967295),        // Skymaze2
        1062 => (8192, 0, 0, 0, 1002, 4294967295),        // Skymaze3
        1063 => (492, 0, 449, 358, 1015, 4294967295),     // SnakeIslands
        1064 => (8192, 0, 0, 0, 1002, 4294967295),        // Star
        1066 => (8192, 0, 0, 0, 1011, 4294967295),        // WoodsZ
        1067 => (8192, 0, 0, 0, 1016, 4294967295),        // QilingZ
        1070 => (8192, 0, 0, 0, 1002, 4294967295),        // Boa
        1075 => (8192, 3, 566, 565, 1020, 4294967295),    // Newcanyon
        1076 => (8192, 3, 301, 488, 1076, 4294967295),    // Newwoods
        1077 => (8192, 2, 447, 347, 1077, 4294967295),    // Newdesert
        1078 => (8192, 0, 0, 0, 1015, 4294967295),        // Newisland
        1079 => (8192, 0, 0, 0, 1015, 4294967295),        // MysIsland
        1080 => (8192, 0, 0, 0, 1005, 4294967295),        // PArena
        1082 => (8192, 0, 724, 573, 1015, 4294967295),    // IdlandMap
        1090 => (7, 0, 430, 380, 1002, 4294967295),       // ParenaM
        1091 => (7, 0, 430, 380, 1002, 4294967295),       // ParenaS
        1098 => (8192, 0, 0, 0, 1002, 4294967295),        // House01
        1099 => (8192, 0, 0, 0, 1002, 4294967295),        // House03
        1100 => (8192, 0, 0, 0, 1100, 4294967295),        // Star01
        1101 => (8192, 0, 0, 0, 1101, 4294967295),        // Star02
        1102 => (8192, 0, 0, 0, 1102, 4294967295),        // Star03
        1103 => (8192, 0, 0, 0, 1103, 4294967295),        // Star04
        1104 => (8192, 0, 0, 0, 1104, 4294967295),        // Star05
        1105 => (8192, 0, 0, 0, 1105, 4294967295),        // Star10
        1106 => (8192, 0, 0, 0, 1106, 4294967295),        // Star06
        1107 => (8192, 0, 0, 0, 1107, 4294967295),        // Star07
        1108 => (8192, 0, 0, 0, 1108, 4294967295),        // Star08
        1109 => (8192, 0, 0, 0, 1109, 4294967295),        // Star09
        1201 => (0, 0, 447, 347, 1077, 4294967295),       // Task01
        1202 => (0, 0, 447, 347, 1077, 4294967295),       // Task02
        1204 => (0, 0, 448, 272, 1213, 4294967295),       // Task04
        1205 => (0, 0, 447, 347, 1077, 4294967295),       // Task05
        1207 => (0, 0, 448, 272, 1213, 4294967295),       // Task07
        1208 => (0, 0, 448, 272, 1213, 4294967295),       // Task08
        1210 => (64, 0, 1017, 1294, 1219, 4294967295),    // Task10
        1211 => (64, 0, 1017, 1294, 1219, 4294967295),    // Task11
        1212 => (64, 0, 1017, 1294, 1219, 4294967295),    // IslandSnail
        1213 => (0, 0, 448, 272, 1213, 4294967295),       // DesertSnail
        1214 => (0, 0, 448, 272, 1213, 4294967295),       // CanyonFairy
        1215 => (0, 0, 448, 272, 1213, 4294967295),       // WoodsFairy
        1216 => (0, 0, 448, 272, 1213, 4294967295),       // NewplainFairy
        1217 => (0, 0, 448, 272, 1213, 4294967295),       // Task07 variant
        1218 => (2114, 10, 1003, 1278, 1219, 4294967295), // Task07 Mine
        1219 => (64, 0, 1003, 1278, 1213, 4294967295),    // Adventure-11
        1300 => (8194, 0, 0, 0, 1000, 4294967295),        // MysticCave
        1351 => (8194, 0, 430, 380, 1002, 4294967295),    // STask01
        1352 => (8194, 0, 430, 380, 1002, 4294967295),    // STask02
        1353 => (8194, 0, 430, 380, 1002, 4294967295),    // STask03
        1354 => (8194, 0, 430, 380, 1002, 4294967295),    // STask04
        1500 => (8192, 0, 0, 0, 1002, 4294967295),        // MineA
        1501 => (8192, 0, 0, 0, 1002, 4294967295),        // MineB
        1502 => (8192, 0, 0, 0, 1002, 4294967295),        // MineC
        1503 => (8192, 0, 0, 0, 1002, 4294967295),        // MineD
        1505 => (8192, 0, 0, 0, 1002, 4294967295),        // Slpk
        1506 => (8192, 0, 0, 0, 1002, 4294967295),        // Hhpk
        1507 => (8192, 0, 0, 0, 1002, 4294967295),        // Blpk
        1508 => (8192, 0, 0, 0, 1002, 4294967295),        // Ympk
        1509 => (8192, 0, 0, 0, 1002, 4294967295),        // Mfpk
        1511 => (8200, 0, 430, 380, 1002, 4294967295),    // FurnitureStore
        1550 => (8192, 0, 0, 0, 1038, 4294967295),        // Faction01
        1601 => (8192, 0, 0, 0, 1000, 4294967295),        // Sanctuary
        1615 => (8192, 0, 0, 0, 1000, 4294967295),        // Jokul01
        1616 => (8192, 0, 0, 0, 1000, 4294967295),        // Tiemfiles
        1645 => (8192, 0, 0, 0, 1000, 4294967295),        // Cordiform
        1760 => (8192, 0, 0, 0, 1015, 4294967295),        // Fairylandpk07
        1762 => (8192, 3, 0, 0, 1000, 4294967295),        // IcecryptLev1
        1764 => (8192, 0, 0, 0, 1015, 4294967295),        // Fairylandpk03
        1766 => (8192, 0, 0, 0, 1015, 4294967295),        // Halloween2007A
        1767 => (8192, 0, 0, 0, 1015, 4294967295),        // Halloween2007Boss
        2021 => (8198, 0, 566, 565, 1020, 4294967295),    // Dgate
        2022 => (8198, 0, 566, 565, 1020, 4294967295),    // Dsquare
        2023 => (8198, 0, 566, 565, 1020, 4294967295),    // Dcloister
        2024 => (4, 0, 566, 565, 1020, 4294967295),       // Dsigil
        4000 => (8192, 3, 0, 0, 1000, 4294967295),        // IceCrypt-1
        4001 => (8192, 3, 0, 0, 1000, 4294967295),        // IceCrypt-2
        6000 => (10594, 0, 29, 72, 6000, 4294967295),     // Prison
        6001 => (8266, 0, 29, 72, 6001, 4294967295),      // BotJail
        601 => (74, 0, 65, 55, 601, 4294967295),          // Nhouse04
        700 => (588, 0, 40, 47, 700, 4294967295),         // ArenaNone
        7001 => (101, 0, 51, 50, 7001, 4294967295),       // FreeForAll
        7002 => (613, 0, 51, 50, 7002, 4294967295),       // CageMatch
        _ => {
            let base_flags = if name.contains("PK") || name.contains("Arena") || name.contains("Arena") {
                1
            } else if name.contains("Mine") || name.contains("Cave") {
                10242
            } else if name.contains("Desert")
                || name.contains("Canyon")
                || name.contains("Woods")
                || name.contains("Plain")
                || name.contains("Island")
            {
                24576
            } else {
                8192
            };
            (base_flags, 0, 0, 0, map_id, 4294967295)
        },
    };
    (flags, weather, portal_x, portal_y, reborn_map, color)
}

fn map_name_from_path(path: &str) -> String {
    path.strip_suffix(".cmap")
        .unwrap_or(path)
        .split('/')
        .next_back()
        .unwrap_or(path)
        .to_string()
}

fn main() -> anyhow::Result<()> {
    dotenvy::dotenv()?;
    let data_path = env::var("DATA_LOCATION")?;
    let dat_path = PathBuf::from(&data_path).join("GameMaps").join("GameMap.dat");
    let maps_csv = PathBuf::from(&data_path).join("Maps").join("Maps.csv");
    let portals_csv = PathBuf::from(&data_path).join("Maps").join("Portals.csv");

    let csv_reader = csv::ReaderBuilder::new().has_headers(true).from_path(maps_csv)?;
    let maps_csv_data: HashMap<u32, MapCsv> = csv_reader
        .into_deserialize::<MapCsv>()
        .filter_map(Result::ok)
        .map(|m| (m.id, m))
        .collect();

    let csv_reader = csv::ReaderBuilder::new().has_headers(true).from_path(portals_csv)?;
    let portals = csv_reader
        .into_deserialize::<Portal>()
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    let f = fs::File::open(dat_path)?;
    let mut buffer = Vec::new();
    let mut reader = BufReader::with_capacity(1024, f);
    reader.read_to_end(&mut buffer)?;
    let mut buffer = Bytes::from(buffer);
    let amount = buffer.get_u32_le();

    let mut map_entries = Vec::new();
    for _ in 0..amount {
        let map_id = buffer.get_u32_le();
        let path_len = buffer.get_u32_le() as usize;
        let path_buf = buffer.split_to(path_len);
        let mut path = String::from_utf8(path_buf.into())?;
        path.replace_range(0..8, "");
        let path = path.replace(".DMap", ".cmap");
        buffer.advance(4);

        let full_path = format!("map/{}", path);
        let dmap_path = PathBuf::from(&data_path)
            .join("GameMaps")
            .join(full_path.replace(".cmap", ".DMap"));
        if !dmap_path.exists() {
            continue;
        }

        let name = map_name_from_path(&path);
        let (flags, weather, portal_x, portal_y, reborn_map, color) = maps_csv_data
            .get(&map_id)
            .map(|m| (m.flags, m.weather, m.portal_x, m.portal_y, m.reborn_map, m.color))
            .unwrap_or_else(|| default_metadata(map_id, &name));

        map_entries.push(MapEntry {
            map_id,
            path: full_path,
            name,
            flags,
            weather,
            portal_x,
            portal_y,
            reborn_map,
            color,
        });
    }

    let valid_map_ids: std::collections::HashSet<u32> = map_entries.iter().map(|m| m.map_id).collect();

    for entry in &map_entries {
        println!(
            "INSERT INTO maps VALUES ({}, {}, '{}', {}, {}, {}, {}, {}, {});",
            entry.map_id,
            entry.map_id,
            entry.path,
            entry.portal_x,
            entry.portal_y,
            entry.flags,
            entry.weather,
            entry.reborn_map,
            entry.color
        );
    }

    println!();

    for portal in portals {
        if valid_map_ids.contains(&portal.from_map_id) && valid_map_ids.contains(&portal.to_map_id) {
            println!(
                "INSERT INTO portals VALUES ({}, {}, {}, {}, {}, {}, {});",
                portal.id, portal.from_map_id, portal.from_x, portal.from_y, portal.to_map_id, portal.to_x, portal.to_y
            );
        } else {
            println!(
                "-- INSERT INTO portals VALUES ({}, {}, {}, {}, {}, {}, {});",
                portal.id, portal.from_map_id, portal.from_x, portal.from_y, portal.to_map_id, portal.to_x, portal.to_y
            );
        }
    }

    Ok(())
}
