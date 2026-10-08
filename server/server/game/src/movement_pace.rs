//! How fast a character may move: a speed hack walks or jumps faster than
//! any client can, and the server pulls it back.
//!
//! Each step and jump costs the time the client takes for it (a running
//! step a sixth of a second, a jump half a second or more, both halved in
//! Cyclone, as `Conquer.exe` halves their frame interval). Moves are paced
//! like a token bucket (GCRA): a quarter of each cost is forgiven for
//! clocks and rounding, and up to two seconds may arrive at once, as after
//! a lag spike. A client that keeps ahead of that is moving too fast.

use parking_lot::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

/// A running step: the client runs six cells a second.
pub const STEP: Duration = Duration::from_micros(166_667);
/// The shortest jump the client makes, and the longest (16 cells across):
/// its jump lasts twice the 0.825 s motion times its length's share of the
/// longest, two fifths at least and half a second at least, so 0.66 to
/// 1.65 s; a little less here. The length is measured on the ground as the
/// screen shows it (the isometric world), not in cells: a jump straight up
/// the screen covers a diagonal of cells but less ground than one across.
const SHORTEST_JUMP: Duration = Duration::from_millis(500);
const LONGEST_JUMP: Duration = Duration::from_millis(1_500);
const LONGEST_JUMP_CELLS: f32 = 16.0;

/// A cell offset on the screen's ground (the client's `cell_to_world`).
fn ground_length(dx: f32, dy: f32) -> f32 {
    (dx - dy).hypot((dx + dy) * 0.5)
}

/// What a jump from `from` to `to` costs, as long as the client draws it.
pub fn jump_cost_between(from: (u16, u16), to: (u16, u16)) -> Duration {
    let dx = f32::from(to.0) - f32::from(from.0);
    let dy = f32::from(to.1) - f32::from(from.1);
    let share = ground_length(dx, dy) / ground_length(LONGEST_JUMP_CELLS, 0.0);
    LONGEST_JUMP.mul_f32(share.clamp(0.4, 1.0)).max(SHORTEST_JUMP)
}
/// The share of each move's time a client must take, in percent.
const PACE_PERCENT: u32 = 75;
/// How far ahead of its pace a client may get at once.
const BURST: Duration = Duration::from_secs(2);

/// What a jump of `cells` costs (the client's jump lasts with its length).
pub fn jump_cost(cells: f32) -> Duration {
    LONGEST_JUMP
        .mul_f32((cells / LONGEST_JUMP_CELLS).clamp(0.0, 1.0))
        .max(SHORTEST_JUMP)
}

#[derive(Debug, Default)]
pub struct Pace {
    /// When the character's moves so far would have ended at full pace.
    caught_up_at: Mutex<Option<Instant>>,
    /// Moves refused: the first and every tenth are logged.
    refused: AtomicU32,
}

impl Pace {
    /// Whether a move costing `cost` (`haste` times faster) is in pace at
    /// `now`; one that is not is not counted.
    pub fn admit(&self, cost: Duration, haste: u32, now: Instant) -> bool {
        let cost = cost * PACE_PERCENT / 100 / haste.max(1);
        let mut caught_up_at = self.caught_up_at.lock();
        let from = caught_up_at.map_or(now, |at| at.max(now));
        if from > now + BURST {
            return false;
        }
        *caught_up_at = Some(from + cost);
        true
    }

    /// A teleport or a revive starts afresh.
    pub fn reset(&self) {
        *self.caught_up_at.lock() = None;
    }

    /// One more refused move; whether to log it.
    pub fn refuse(&self) -> bool {
        let refused = self.refused.fetch_add(1, Ordering::Relaxed);
        refused % 10 == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(pace: &Pace, start: Instant, steps: u32, every: Duration, haste: u32) -> u32 {
        (0..steps)
            .filter(|step| pace.admit(STEP, haste, start + every * *step))
            .count() as u32
    }

    #[test]
    fn running_at_full_speed_is_never_refused() {
        let pace = Pace::default();
        assert_eq!(run(&pace, Instant::now(), 600, STEP, 1), 600, "a hundred seconds of running");
    }

    #[test]
    fn running_twice_as_fast_is_caught() {
        let pace = Pace::default();
        let admitted = run(&pace, Instant::now(), 120, STEP / 2, 1);
        assert!(admitted < 100, "{admitted} of 120 steps at twice the speed");
    }

    #[test]
    fn cyclone_runs_twice_as_fast() {
        let pace = Pace::default();
        assert_eq!(run(&pace, Instant::now(), 600, STEP / 2, 2), 600);
    }

    #[test]
    fn steps_held_up_by_a_lag_spike_arrive_together() {
        let pace = Pace::default();
        let start = Instant::now();
        assert_eq!(run(&pace, start, 30, STEP, 1), 30);
        // Two seconds without packets, then the twelve steps of them at once.
        let late = start + STEP * 30 + Duration::from_secs(2);
        let admitted = (0..12).filter(|_| pace.admit(STEP, 1, late)).count();
        assert_eq!(admitted, 12);
    }

    #[test]
    fn jumps_cost_what_the_client_draws() {
        // Across the screen (along a cell axis) as the cells say; straight
        // up the screen, a diagonal of cells, less: the ground is shorter.
        let across = jump_cost_between((100, 100), (116, 100));
        let up = jump_cost_between((100, 100), (89, 89));
        assert_eq!(across, LONGEST_JUMP);
        assert!(up < jump_cost(f32::from(11_u16).hypot(11.0)), "{up:?}");
        // Long jumps straight up the screen, one as each lands, go on.
        let pace = Pace::default();
        let start = Instant::now();
        let mut at = start;
        for _ in 0..60 {
            let cost = jump_cost_between((100, 100), (89, 89));
            assert!(pace.admit(cost, 1, at));
            at += cost.mul_f32(1.65 / 1.5);
        }
    }

    #[test]
    fn jumps_cost_with_their_length() {
        assert_eq!(jump_cost(2.0), SHORTEST_JUMP);
        assert_eq!(jump_cost(16.0), LONGEST_JUMP);
        let pace = Pace::default();
        let start = Instant::now();
        // A long jump every 1.5 s goes on; one every 0.2 s does not.
        assert!((0..60).all(|jump| pace.admit(jump_cost(16.0), 1, start + LONGEST_JUMP * jump)));
        let pace = Pace::default();
        let hops = (0..60)
            .filter(|jump| pace.admit(jump_cost(16.0), 1, start + Duration::from_millis(200) * *jump))
            .count();
        assert!(hops < 30, "{hops} long jumps of 60, 0.2 s apart");
    }
}
