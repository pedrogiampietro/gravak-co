//! The website's Armory: a hero as they are seen in the world, turned all
//! the way around, drawn without a window into one sheet of frames side by
//! side that the page turns with the mouse.
//!
//! `conquer-sandbox --armory <out.png> <mesh> <hair> <armor> <garment>
//! <helmet> <right> <left> [--assets <dir>]`: the ids as `characters` and
//! `equipment_items` keep them, 0 for an empty slot.

use crate::Result;

/// How many turns of the hero the sheet holds, and each frame's size.
pub const FRAMES: u32 = 24;
pub const FRAME: [u32; 2] = [400, 460];
/// The first frame faces the viewer a little turned, as the site's art.
const FIRST_FACING: f32 = 0.35;

pub fn run(mut args: impl Iterator<Item = std::ffi::OsString>) -> Result<()> {
    let mut words = Vec::new();
    let mut root = None;
    while let Some(arg) = args.next() {
        if arg == "--assets" {
            root = Some(std::path::PathBuf::from(args.next().ok_or("--assets requires a directory")?));
        } else {
            words.push(arg.into_string().map_err(|_| "Invalid argument encoding")?);
        }
    }
    let [out, numbers @ ..] = words.as_slice() else {
        return Err("--armory requires <out.png> <mesh> <hair> <armor> <garment> <helmet> <right> <left>".into());
    };
    let numbers = numbers.iter().map(|word| word.parse::<u32>()).collect::<std::result::Result<Vec<_>, _>>()?;
    let [mesh, hair, armor, garment, helmet, right, left] = numbers[..] else {
        return Err("--armory requires seven ids after the file".into());
    };
    let root = root
        .or_else(|| crate::settings::load_client_config().and_then(|config| config.assets))
        .or_else(|| std::env::var_os("GRAVAK_ASSETS").map(Into::into))
        .ok_or("--armory requires --assets <dir> (or GRAVAK_ASSETS)")?;
    let mut assets = crate::assets::Assets::open(&root)?;
    let body = crate::network::body_from_mesh(mesh);
    if body == 0 {
        return Err(format!("mesh {mesh} has no body").into());
    }
    // A garment is worn over the armor, as in the world.
    let armor = if garment > 0 { garment } else { armor };
    let hair = u16::try_from(hair).unwrap_or(crate::showcase::HAIR);
    let hero = crate::showcase::Hero::load_worn(&mut assets, body, [armor, helmet, right, left], hair).ok_or("the hero cannot be drawn")?;
    sheet(&hero).save(out)?;
    Ok(())
}

/// Every frame, turning clockwise as seen from above, left to right.
fn sheet(hero: &crate::showcase::Hero) -> image::RgbaImage {
    let mut sheet = image::RgbaImage::new(FRAME[0] * FRAMES, FRAME[1]);
    for frame in 0..FRAMES {
        let facing = FIRST_FACING + std::f32::consts::TAU * frame as f32 / FRAMES as f32;
        let picture = hero.render(FRAME, FRAME[1] as f32 * 0.74, 0.6, facing);
        image::imageops::overlay(&mut sheet, &picture, i64::from(frame * FRAME[0]), 0);
    }
    sheet
}
