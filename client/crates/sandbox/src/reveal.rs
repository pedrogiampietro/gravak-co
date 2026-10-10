//! Discovering a Rift's crypt room by room. A room the hero has not seen is
//! dark: no floor, no walls, nobody in it. As soon as their line of sight
//! reaches into a room the whole room comes out of the dark, fading in, and
//! stays; the walls round it come with it. The passages between rooms come
//! out cell by cell, as they are seen. What has come out is drawn as on any
//! other map.

use crate::map::Map;

/// How far the hero sees down a corridor, in cells.
const SIGHT: i32 = 12;
/// Blocked cells (walls, pillars, chests) this near a cell that comes out
/// come out with it: a wall is two cells thick.
const WALL_REACH: i32 = 2;
/// How long a room takes to come out of the dark, in seconds.
const FADE_SECONDS: f32 = 0.45;

pub struct Reveal {
    pub map_id: u32,
    size: [i32; 2],
    open: Vec<bool>,
    room_of: Vec<Option<usize>>,
    rooms: Vec<Vec<[i32; 2]>>,
    /// When each cell came out of the dark, in seconds; `NaN` while dark.
    shown_at: Vec<f32>,
    from: Option<[i32; 2]>,
    /// Counts what came out, so the minimap knows when to redraw.
    pub generation: u32,
}

impl Reveal {
    /// The crypt of `map`, all dark; none for a map without rooms.
    pub fn new(map: &Map) -> Option<Self> {
        if map.rooms.is_empty() {
            return None;
        }
        let size = [map.data.width as i32, map.data.height as i32];
        let cells = (size[0] * size[1]) as usize;
        let open = (0..cells).map(|index| map.walkable([index as i32 % size[0], index as i32 / size[0]])).collect();
        let mut reveal = Self {
            map_id: map.id,
            size,
            open,
            room_of: vec![None; cells],
            rooms: map.rooms.clone(),
            shown_at: vec![f32::NAN; cells],
            from: None,
            generation: 0,
        };
        for (room, cells) in map.rooms.iter().enumerate() {
            for &cell in cells {
                if let Some(index) = reveal.index(cell) {
                    reveal.room_of[index] = Some(room);
                }
            }
        }
        Some(reveal)
    }

    fn index(&self, [x, y]: [i32; 2]) -> Option<usize> {
        (x >= 0 && y >= 0 && x < self.size[0] && y < self.size[1]).then(|| (y * self.size[0] + x) as usize)
    }

    fn open(&self, cell: [i32; 2]) -> bool {
        self.index(cell).is_some_and(|index| self.open[index])
    }

    /// The hero stands on `hero` at `time`: what they see from there comes
    /// out. Only worked out when they step onto another cell.
    pub fn look(&mut self, hero: [i32; 2], time: f32) {
        if self.from == Some(hero) {
            return;
        }
        self.from = Some(hero);
        let mut rooms = Vec::new();
        let mut passages = Vec::new();
        for dy in -SIGHT..=SIGHT {
            for dx in -SIGHT..=SIGHT {
                if dx * dx + dy * dy > SIGHT * SIGHT {
                    continue;
                }
                let target = [hero[0] + dx, hero[1] + dy];
                let Some(index) = self.index(target) else { continue };
                if !self.open[index] || !self.in_sight(hero, target) {
                    continue;
                }
                match self.room_of[index] {
                    Some(room) if !rooms.contains(&room) => rooms.push(room),
                    Some(_) => {}
                    None => passages.push(target),
                }
            }
        }
        for room in rooms {
            for cell in self.rooms[room].clone() {
                self.show(cell, time);
            }
        }
        for cell in passages {
            self.show(cell, time);
        }
    }

    /// `cell` comes out at `time`, the blocked cells round it with it.
    fn show(&mut self, cell: [i32; 2], time: f32) {
        for dy in -WALL_REACH..=WALL_REACH {
            for dx in -WALL_REACH..=WALL_REACH {
                let near = [cell[0] + dx, cell[1] + dy];
                let Some(index) = self.index(near) else { continue };
                if (near == cell || !self.open[index]) && self.shown_at[index].is_nan() {
                    self.shown_at[index] = time;
                    self.generation += 1;
                }
            }
        }
    }

    /// How much of `cell` shows at `time`: 0 dark, 1 out.
    pub fn shown(&self, cell: [i32; 2], time: f32) -> f32 {
        match self.index(cell).map(|index| self.shown_at[index]) {
            Some(at) if !at.is_nan() => ((time - at) / FADE_SECONDS).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }

    /// Whether `cell` has come out (or is coming out).
    pub fn seen(&self, cell: [i32; 2]) -> bool {
        self.index(cell).is_some_and(|index| !self.shown_at[index].is_nan())
    }

    /// Whether `to` can be seen from `from`: every cell between them open.
    fn in_sight(&self, from: [i32; 2], to: [i32; 2]) -> bool {
        let (dx, dy) = ((to[0] - from[0]).abs(), -(to[1] - from[1]).abs());
        let (sx, sy) = ((to[0] - from[0]).signum(), (to[1] - from[1]).signum());
        let (mut x, mut y) = (from[0], from[1]);
        let mut error = dx + dy;
        loop {
            if [x, y] == to {
                return true;
            }
            if [x, y] != from && !self.open([x, y]) {
                return false;
            }
            let twice = 2 * error;
            if twice >= dy {
                error += dy;
                x += sx;
            }
            if twice <= dx {
                error += dx;
                y += sy;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A crypt drawn in text: `.` floor, `#` wall; rooms are the runs of
    /// floor of each letter (`a`, `b`...) and `.` a passage.
    fn crypt(rows: &[&str]) -> Reveal {
        let size = [rows[0].len() as i32, rows.len() as i32];
        let cells = (size[0] * size[1]) as usize;
        let mut open = vec![false; cells];
        let mut room_of = vec![None; cells];
        let mut rooms: Vec<Vec<[i32; 2]>> = Vec::new();
        let mut letters = Vec::new();
        for (y, row) in rows.iter().enumerate() {
            for (x, c) in row.chars().enumerate() {
                let index = y * size[0] as usize + x;
                if c == '#' {
                    continue;
                }
                open[index] = true;
                if c.is_ascii_lowercase() {
                    let room = letters.iter().position(|&l| l == c).unwrap_or_else(|| {
                        letters.push(c);
                        rooms.push(Vec::new());
                        letters.len() - 1
                    });
                    rooms[room].push([x as i32, y as i32]);
                    room_of[index] = Some(room);
                }
            }
        }
        Reveal { map_id: 0, size, open, room_of, rooms, shown_at: vec![f32::NAN; cells], from: None, generation: 0 }
    }

    const CRYPT: [&str; 7] = [
        "##########",
        "#aaa##bbb#",
        "#aaa..bbb#",
        "#aaa##bbb#",
        "###.######",
        "#ccc######",
        "##########",
    ];

    #[test]
    fn a_room_comes_out_whole_when_seen() {
        let mut reveal = crypt(&CRYPT);
        reveal.look([2, 2], 0.0);
        // The hero's room, all of it, and its walls.
        assert!(reveal.seen([3, 3]) && reveal.seen([0, 0]));
        // Room b is seen through the passage: all of it comes out, the
        // corner the hero cannot see too.
        assert!(reveal.seen([8, 1]) && reveal.seen([8, 3]));
    }

    #[test]
    fn what_is_out_of_sight_stays_dark() {
        let mut reveal = crypt(&CRYPT);
        reveal.look([8, 1], 0.0);
        // Room c, round two corners from room b, is never in sight.
        assert!(!reveal.seen([2, 5]));
        assert_eq!(reveal.shown([2, 5], 10.0), 0.0);
    }

    #[test]
    fn rooms_fade_in_and_stay() {
        let mut reveal = crypt(&CRYPT);
        reveal.look([1, 1], 1.0);
        assert_eq!(reveal.shown([2, 2], 1.0), 0.0);
        assert!(reveal.shown([2, 2], 1.0 + FADE_SECONDS / 2.0) > 0.4);
        assert_eq!(reveal.shown([2, 2], 1.0 + FADE_SECONDS), 1.0);
        // Walking on, what was seen stays out.
        reveal.look([3, 4], 2.0);
        assert_eq!(reveal.shown([1, 1], 5.0), 1.0);
        assert!(reveal.seen([2, 5]), "room c below the passage is in sight now");
    }
}

/// A hero walking the Covil dos Goblins (map 9091) from its entrance to its
/// farthest room, and what they have discovered at four points of the way:
/// `marketing/fendas/descoberta.png` (floor out: sand; floor dark: blue;
/// walls out: brown; dark: black; the way: red). `GRAVAK_CONTENT=...
/// cargo test -p conquer-sandbox walk_the_covil -- --ignored`.
#[cfg(test)]
#[test]
#[ignore = "Needs the 5065 installation and the built content"]
fn walk_the_covil() {
    use std::collections::VecDeque;
    let mut assets = crate::assets::Assets::open(std::path::Path::new("R:/5065")).expect("assets");
    let map = Map::load(&mut assets, 9091).expect("map");
    assert!(!map.rooms.is_empty(), "the Covil's rooms (gravak/rifts/9091.txt)");
    let mut reveal = Reveal::new(&map).expect("a crypt");
    let [width, height] = reveal.size;
    // The way: the shortest walk from the entrance to the farthest cell.
    let start = map.rooms[0][map.rooms[0].len() / 2];
    let mut came_from = vec![None; (width * height) as usize];
    let mut queue = VecDeque::from([start]);
    let mut last = start;
    came_from[reveal.index(start).unwrap()] = Some(start);
    while let Some(cell) = queue.pop_front() {
        last = cell;
        for step in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
            let next = [cell[0] + step[0], cell[1] + step[1]];
            if let Some(index) = reveal.index(next) {
                if reveal.open[index] && came_from[index].is_none() {
                    came_from[index] = Some(cell);
                    queue.push_back(next);
                }
            }
        }
    }
    let mut way = vec![last];
    while let Some(previous) = came_from[reveal.index(*way.last().unwrap()).unwrap()] {
        if previous == *way.last().unwrap() {
            break;
        }
        way.push(previous);
    }
    way.reverse();
    const CELL: u32 = 5;
    let frame = |reveal: &Reveal, walked: &[[i32; 2]]| {
        let mut picture = image::RgbaImage::from_pixel(width as u32 * CELL, height as u32 * CELL, image::Rgba([0, 0, 0, 255]));
        for y in 0..height {
            for x in 0..width {
                let index = reveal.index([x, y]).unwrap();
                let colour = match (reveal.open[index], reveal.seen([x, y])) {
                    (true, true) => [214, 190, 140],
                    (true, false) => [30, 40, 70],
                    (false, true) => [110, 70, 35],
                    (false, false) => continue,
                };
                for py in 0..CELL {
                    for px in 0..CELL {
                        picture.put_pixel(x as u32 * CELL + px, y as u32 * CELL + py, image::Rgba([colour[0], colour[1], colour[2], 255]));
                    }
                }
            }
        }
        for cell in walked {
            for py in 1..CELL - 1 {
                for px in 1..CELL - 1 {
                    picture.put_pixel(cell[0] as u32 * CELL + px, cell[1] as u32 * CELL + py, image::Rgba([220, 40, 40, 255]));
                }
            }
        }
        picture
    };
    let stops = [way.len() / 10, way.len() / 3, way.len() * 2 / 3, way.len() - 1];
    let mut sheet = image::RgbaImage::from_pixel(width as u32 * CELL * 4 + 30, height as u32 * CELL, image::Rgba([60, 60, 60, 255]));
    let mut taken = 0;
    for (index, cell) in way.iter().enumerate() {
        reveal.look(*cell, index as f32);
        if stops.contains(&index) {
            let picture = frame(&reveal, &way[..=index]);
            image::imageops::overlay(&mut sheet, &picture, i64::from(taken * (width as u32 * CELL + 10)), 0);
            let open = reveal.open.iter().filter(|open| **open).count();
            let seen = (0..reveal.open.len()).filter(|&i| reveal.open[i] && !reveal.shown_at[i].is_nan()).count();
            println!("step {index}/{}: {seen} of {open} floor cells discovered", way.len());
            taken += 1;
        }
    }
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../marketing/fendas");
    std::fs::create_dir_all(&out).unwrap();
    sheet.save(out.join("descoberta.png")).unwrap();
    // The minimap at the end of the walk: only what was discovered.
    let mut minimap = crate::minimap::Minimap::load(&mut assets, 9091).expect("minimap").expect("a minimap");
    minimap.discovered(&map, |cell| reveal.seen(cell));
    let full = &minimap.images.iter().find(|(key, _)| *key == "@minimap-full").unwrap().1;
    full.save(out.join("descoberta-minimapa.png")).unwrap();
}
