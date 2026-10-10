//! The fog over the Rifts' crypts: the hero sees only what is in their line
//! of sight, the walls blocking it; what they have seen stays dim, what they
//! have never seen is dark. Monsters and other players out of sight are not
//! drawn, so a turn of the corridor can hide anything.

use crate::map::Map;

/// How far a hero sees down a corridor, in cells.
const SIGHT: i32 = 12;

/// The maps under fog: the crypts of `content/rifts.toml` (9091–9129).
pub fn covers(map_id: u32) -> bool {
    (9_091..=9_129).contains(&map_id)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sight {
    Visible,
    Seen,
    Unknown,
}

#[derive(Clone)]
pub struct Fog {
    pub map_id: u32,
    size: [i32; 2],
    seen: Vec<bool>,
    visible: Vec<bool>,
    from: Option<[i32; 2]>,
}

impl Fog {
    pub fn new(map: &Map) -> Self {
        let size = [map.data.width as i32, map.data.height as i32];
        let cells = (size[0] * size[1]) as usize;
        Self { map_id: map.id, size, seen: vec![false; cells], visible: vec![false; cells], from: None }
    }

    fn index(&self, [x, y]: [i32; 2]) -> Option<usize> {
        (x >= 0 && y >= 0 && x < self.size[0] && y < self.size[1]).then(|| (y * self.size[0] + x) as usize)
    }

    /// The hero stands on `hero`: what they see from there. Only worked out
    /// when they step onto another cell.
    pub fn look(&mut self, map: &Map, hero: [i32; 2]) {
        if self.from == Some(hero) {
            return;
        }
        self.from = Some(hero);
        self.visible.iter_mut().for_each(|cell| *cell = false);
        for dy in -SIGHT..=SIGHT {
            for dx in -SIGHT..=SIGHT {
                if dx * dx + dy * dy > SIGHT * SIGHT {
                    continue;
                }
                let target = [hero[0] + dx, hero[1] + dy];
                let Some(index) = self.index(target) else { continue };
                if in_sight(map, hero, target) {
                    self.visible[index] = true;
                    self.seen[index] = true;
                }
            }
        }
    }

    pub fn sight(&self, cell: [i32; 2]) -> Sight {
        match self.index(cell) {
            Some(index) if self.visible[index] => Sight::Visible,
            Some(index) if self.seen[index] => Sight::Seen,
            _ => Sight::Unknown,
        }
    }

    /// Whether whatever stands at `position` (cells) is in sight.
    pub fn shows(&self, position: [f32; 2]) -> bool {
        self.sight([position[0].round() as i32, position[1].round() as i32]) == Sight::Visible
    }
}

/// Whether `to` can be seen from `from`: every cell between them open (`to`
/// itself may be a wall: its face is what is seen).
fn in_sight(map: &Map, from: [i32; 2], to: [i32; 2]) -> bool {
    let (dx, dy) = ((to[0] - from[0]).abs(), -(to[1] - from[1]).abs());
    let (sx, sy) = ((to[0] - from[0]).signum(), (to[1] - from[1]).signum());
    let (mut x, mut y) = (from[0], from[1]);
    let mut error = dx + dy;
    loop {
        if [x, y] == to {
            return true;
        }
        if [x, y] != from && !map.walkable([x, y]) {
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
