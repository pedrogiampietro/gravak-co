// Source: @digitalm1nd on x.com / _dek on Discord - Preservation Conquer project - https://discord.gg/CvKPXEHYRY
use crate::{
    action::Action,
    assets::{Assets, Result},
    character::Character,
    movement::Player,
};
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Source, buffer::SamplesBuffer};
use sandbox_data::audio::{ActionSoundCatalog, ActionSoundKey};
use std::{collections::HashMap, io::Cursor};

pub struct MovementAudio {
    device: Option<MixerDeviceSink>,
    paths: [Option<String>; 11],
    clips: HashMap<String, SamplesBuffer>,
    voices: HashMap<String, rodio::Player>,
    last_action: Option<(Action, u64)>,
    /// Sound effect volume, 0.0..=1.0.
    sound_volume: f32,
    music: MusicPlayer,
}

/// A `ini/MusicRegion.ini` area: the track that loops while the hero is in
/// it.
struct MusicRegion {
    map_id: u32,
    bounds: [i32; 4],
    track: String,
}

/// Background music: the region's track, decoded off the render thread and
/// looped at the music volume.
#[derive(Default)]
struct MusicPlayer {
    regions: Vec<MusicRegion>,
    /// The track asked for, and the one playing.
    wanted: Option<String>,
    playing: Option<(String, rodio::Player)>,
    decoded: Option<std::sync::mpsc::Receiver<(String, std::result::Result<SamplesBuffer, String>)>>,
    volume: f32,
}

fn music_regions(assets: &mut Assets) -> Vec<MusicRegion> {
    let Ok(bytes) = assets.read("ini/MusicRegion.ini") else {
        return Vec::new();
    };
    let catalog = sandbox_data::ini::SectionCatalog::parse(&bytes);
    catalog
        .sections()
        .iter()
        .filter_map(|section| {
            let number = |key: &str| section.first(key)?.trim().parse::<i32>().ok();
            let track = section.first("Music0")?.trim().replace('\\', "/");
            (!track.eq_ignore_ascii_case("NULL")).then_some(())?;
            Some(MusicRegion {
                map_id: u32::try_from(number("Mapid")?).ok()?,
                bounds: [number("Bound_X")?, number("Bound_Y")?, number("Bound_CX")?, number("Bound_CY")?],
                track,
            })
        })
        .collect()
}

impl MovementAudio {
    pub fn load(assets: &mut Assets, character: &Character) -> Result<Self> {
        let catalog = ActionSoundCatalog::parse(
            &assets.read("ini/sound.ini").unwrap_or_default(),
            &assets.read("ini/ActionSound.ini")?,
        );
        let paths = Action::ALL.map(|action| {
            action_sound(&catalog, character.body_id, character.weapon_action, action)
                .map(str::to_owned)
        });
        let mut clips = HashMap::new();
        for path in paths.iter().flatten() {
            if clips.contains_key(path) {
                continue;
            }
            let decoded = assets.read(path).and_then(decode_audio_bytes);
            match decoded {
                Ok(clip) => {
                    clips.insert(path.clone(), clip);
                }
                Err(error) => eprintln!("Audio: {path}: {error}"),
            }
        }
        Ok(Self {
            device: None,
            paths,
            clips,
            voices: HashMap::new(),
            last_action: None,
            sound_volume: 0.8,
            music: MusicPlayer {
                regions: music_regions(assets),
                volume: 0.6,
                ..MusicPlayer::default()
            },
        })
    }

    /// Applies the Options window's volumes (0..=100).
    pub fn set_volumes(&mut self, music: u8, sound: u8) {
        self.sound_volume = f32::from(sound.min(100)) / 100.0;
        self.music.volume = f32::from(music.min(100)) / 100.0;
        if let Some((_, player)) = &self.music.playing {
            player.set_volume(self.music.volume);
        }
        for voice in self.voices.values() {
            voice.set_volume(self.sound_volume);
        }
    }

    /// Plays the music of the region the hero stands in, changing track
    /// when the hero walks into another region.
    pub fn update_music(&mut self, assets: &mut Assets, map_id: u32, cell: [i32; 2]) {
        let Some(device) = &self.device else { return };
        let music = &mut self.music;
        if let Some(receiver) = &music.decoded
            && let Ok((track, decoded)) = receiver.try_recv()
        {
            music.decoded = None;
            match decoded {
                Ok(clip) if music.wanted.as_deref() == Some(track.as_str()) => {
                    let player = rodio::Player::connect_new(device.mixer());
                    player.set_volume(music.volume);
                    player.append(clip.repeat_infinite());
                    music.playing = Some((track, player));
                }
                Ok(_) => {}
                Err(error) => eprintln!("Music {track}: {error}"),
            }
        }
        let wanted = music
            .regions
            .iter()
            .find(|region| {
                let [x, y, width, height] = region.bounds;
                region.map_id == map_id
                    && (x..x + width).contains(&cell[0])
                    && (y..y + height).contains(&cell[1])
            })
            .map(|region| region.track.clone());
        if wanted == music.wanted {
            return;
        }
        music.wanted = wanted.clone();
        if let Some((_, player)) = music.playing.take() {
            player.stop();
        }
        let Some(track) = wanted else { return };
        let Ok(bytes) = assets.read(&track) else {
            eprintln!("Music {track} is not in the client data");
            return;
        };
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let decoded = decode_audio_bytes(bytes).map_err(|error| error.to_string());
            let _ = sender.send((track, decoded));
        });
        music.decoded = Some(receiver);
    }

    pub fn open_device(&mut self) {
        match DeviceSinkBuilder::from_default_device()
            .and_then(|builder| builder.open_sink_or_fallback())
        {
            Ok(mut device) => {
                device.log_on_drop(false);
                self.device = Some(device);
            }
            Err(error) => eprintln!("Audio unavailable: {error}"),
        }
    }

    pub fn update(&mut self, player: &Player) {
        let action = (player.action, player.action_instance);
        if self.last_action == Some(action) {
            return;
        }
        self.last_action = Some(action);
        let Some(device) = &self.device else { return };
        let Some(path) = &self.paths[player.action.index()] else {
            return;
        };
        let Some(clip) = self.clips.get(path) else {
            return;
        };
        if let Some(previous) = self.voices.remove(path) {
            previous.stop();
        }
        let voice = rodio::Player::connect_new(device.mixer());
        voice.set_volume(self.sound_volume);
        voice.append(clip.clone());
        self.voices.insert(path.clone(), voice);
    }

    pub fn check(&self) -> Result<()> {
        for path in self.paths.iter().flatten() {
            if !self.clips.contains_key(path) {
                return Err(format!("Could not decode movement sound {path}").into());
            }
        }
        println!("Audio OK: {} decoded movement sounds", self.clips.len());
        Ok(())
    }
}

fn action_sound(
    catalog: &ActionSoundCatalog,
    body_id: u32,
    weapon_id: u32,
    action: Action,
) -> Option<&str> {
    let action_id = action.catalog_id();
    catalog
        .get(ActionSoundKey {
            body_id,
            weapon_id,
            action_id,
        })
        .or_else(|| {
            catalog.get(ActionSoundKey {
                body_id,
                weapon_id: 999,
                action_id,
            })
        })
}

fn decode_audio_bytes(bytes: Vec<u8>) -> Result<SamplesBuffer> {
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("Audio exceeds 32 MiB safety limit".into());
    }
    let decoder = Decoder::try_from(Cursor::new(bytes))?;
    let channels = decoder.channels();
    let sample_rate = decoder.sample_rate();
    let samples = decoder.collect::<Vec<_>>();
    Ok(SamplesBuffer::new(channels, sample_rate, samples))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equipped_sound_precedes_generic_and_absence_stays_silent() {
        let catalog = ActionSoundCatalog::parse_action_sound(
            b"3.611.130=sound/equipped.wav\n3.999.130=sound/jump.wav\n3.999.120=sound/runL.wav\n3.999.121=sound/runR.wav\n"
        );
        assert_eq!(
            action_sound(&catalog, 3, 611, Action::Jump),
            Some("sound/equipped.wav")
        );
        assert_eq!(
            action_sound(&catalog, 3, 410, Action::Jump),
            Some("sound/jump.wav")
        );
        assert_eq!(
            action_sound(&catalog, 3, 611, Action::RunLeft),
            Some("sound/runl.wav")
        );
        assert_eq!(
            action_sound(&catalog, 3, 611, Action::RunRight),
            Some("sound/runr.wav")
        );
        assert_eq!(action_sound(&catalog, 3, 611, Action::Idle), None);
        assert!(decode_audio_bytes(vec![0; 16]).is_err());
    }
}
