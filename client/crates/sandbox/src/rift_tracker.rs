//! The Rift tracker under the minimap (and under the bounty's): the rift's
//! name, the creatures slain of all with a bar, and the time left; then the
//! Guardian, the victory or the defeat. The server sends where the run
//! stands (`MsgRift`, 9180) on every change; the time counts down here.

use crate::ui_theme;
use crate::{hud, labels};
use fontdue::Font;
use gravak_protocol::RiftState;
use image::RgbaImage;

pub const SIZE: [u32; 2] = [206, 58];

/// What the tracker shows: the run and its seconds left now.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Tracker {
    pub phase: u32,
    pub tier: u32,
    pub killed: u32,
    pub total: u32,
    pub seconds_left: u32,
}

impl Tracker {
    /// The run as it stands `elapsed` seconds after the server said so;
    /// none when there is no run.
    pub fn now(state: &RiftState, elapsed: f32) -> Option<Self> {
        (state.phase != 0).then(|| Self {
            phase: state.phase,
            tier: state.tier,
            killed: state.killed,
            total: state.total,
            seconds_left: state.seconds_left.saturating_sub(elapsed as u32),
        })
    }
}

fn tier_name(tier: u32) -> &'static str {
    match tier {
        0 => "FENDA MENOR",
        1 => "FENDA MAIOR",
        2 => "FENDA ABISSAL",
        _ => "COVIL DOS GOBLINS",
    }
}

fn clock(seconds: u32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

pub fn image(assets: &mut crate::assets::Assets, tracker: &Tracker) -> crate::assets::Result<RgbaImage> {
    let [width, height] = SIZE;
    let (frame, accent): ([u8; 3], [u8; 3]) = match tracker.phase {
        2 => ([196, 72, 60], [255, 120, 96]),
        3 => ([226, 190, 96], [255, 214, 120]),
        4 => ([110, 110, 120], [170, 170, 180]),
        _ => ([120, 150, 200], [160, 206, 255]),
    };
    let mut image = hud::hud_style_panel(assets, width, height, frame);
    let cinzel = labels::decorative_font()?;
    let ui = labels::font()?;
    let shadow = [12, 22, 28];
    let write = |image: &mut RgbaImage, font: &Font, text: &str, colour: [u8; 3], x: f32, y: f32, size: f32| {
        labels::text_sized(image, font, &[(text, shadow)], [x + 1.0, y + 1.0], size);
        labels::text_sized(image, font, &[(text, colour)], [x, y], size);
    };
    let width_of = |font: &Font, text: &str, size: f32| text.chars().map(|c| font.metrics(c, size).advance_width).sum::<f32>();
    let (left, right) = (12.0, width as f32 - 12.0);
    write(&mut image, &cinzel, tier_name(tracker.tier), accent, left, 7.0, 13.0);
    let time = clock(tracker.seconds_left);
    let time_colour = if tracker.phase <= 2 && tracker.seconds_left < 120 { [255, 110, 90] } else { ui_theme::TEXT };
    write(&mut image, &ui, &time, time_colour, right - width_of(&ui, &time, 12.0), 9.0, 12.0);
    match tracker.phase {
        1 => {
            let count = format!("Creatures {}/{}", tracker.killed, tracker.total);
            write(&mut image, &ui, &count, ui_theme::TEXT, left, 26.0, 11.0);
            crate::components::draw_bar(
                &mut image,
                crate::ui::UiRect::new(left as i32, 42, (right - left) as i32, 8),
                tracker.killed,
                tracker.total.max(1),
                [92, 140, 210, 255],
                [10, 18, 22, 255],
                [70, 84, 110, 255],
            );
        },
        2 => {
            write(&mut image, &cinzel, "The Guardian is awake!", accent, left, 28.0, 12.0);
        },
        3 => {
            write(&mut image, &cinzel, "Victory!", accent, left, 26.0, 14.0);
            write(&mut image, &ui, "The rift closes soon", ui_theme::TEXT_MUTED, left + 72.0, 30.0, 10.0);
        },
        _ => {
            write(&mut image, &cinzel, "The rift is lost", accent, left, 28.0, 12.0);
        },
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_time_counts_down_from_what_the_server_said() {
        let state = RiftState { phase: 1, tier: 0, killed: 3, total: 228, seconds_left: 1500 };
        assert_eq!(Tracker::now(&state, 61.5).map(|tracker| tracker.seconds_left), Some(1439));
        assert_eq!(Tracker::now(&state, 9999.0).map(|tracker| tracker.seconds_left), Some(0));
        assert!(Tracker::now(&RiftState::default(), 0.0).is_none(), "phase 0 hides it");
        assert_eq!(clock(1439), "23:59");
    }
}
