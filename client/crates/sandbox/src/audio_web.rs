//! The browser build's sound (docs/web-spike.md): the same API as
//! `audio.rs`, played through the page's Web Audio. The browser decodes the
//! game's MP3 and WAV files itself (`decodeAudioData`), off the page's loop,
//! so no decoder and no thread is needed here. A page may only sound after
//! the player's first click or key: the context waits for it and resumes.
//!
//! A file still downloading (`web_files`) is read again the next time it is
//! wanted, as everything else in the browser.
use crate::{
    action::Action,
    assets::{Assets, Result},
    character::Character,
    movement::Player,
};
use sandbox_data::audio::{ActionSoundCatalog, ActionSoundKey};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};
use wasm_bindgen::{JsCast, closure::Closure};
use wasm_bindgen_futures::JsFuture;
use web_sys::{AudioBuffer, AudioBufferSourceNode, AudioContext, GainNode};

/// How long a track takes to rise or to fade away, as natively.
const MUSIC_FADE: f64 = 0.8;

thread_local! {
    static CONTEXT: RefCell<Option<AudioContext>> = const { RefCell::new(None) };
}

/// The page's one audio context, made at the first sound. It starts
/// suspended until the player clicks or presses a key, when it resumes.
fn context() -> Option<AudioContext> {
    CONTEXT.with(|context| {
        if let Some(context) = &*context.borrow() {
            return Some(context.clone());
        }
        let made = AudioContext::new().ok()?;
        let resume = made.clone();
        let wake = Closure::<dyn FnMut()>::new(move || {
            let _ = resume.resume();
        });
        if let Some(document) = web_sys::window().and_then(|window| window.document()) {
            for kind in ["pointerdown", "keydown"] {
                let _ = document.add_event_listener_with_callback(kind, wake.as_ref().unchecked_ref());
            }
        }
        // Kept for the page's life, as the context.
        wake.forget();
        *context.borrow_mut() = Some(made.clone());
        Some(made)
    })
}

fn gain(context: &AudioContext, volume: f32) -> Option<GainNode> {
    let node = context.create_gain().ok()?;
    node.gain().set_value(volume);
    node.connect_with_audio_node(&context.destination()).ok()?;
    Some(node)
}

/// A clip as it comes: bytes being decoded, ready, or not to be had.
enum Clip {
    Decoding,
    Ready(AudioBuffer),
    Failed,
}

/// Clips by path, decoded by the browser as they are asked for.
#[derive(Clone, Default)]
struct Clips(Rc<RefCell<HashMap<String, Clip>>>);

impl Clips {
    /// The clip of `path`, its decoding started if it is new. `None` while
    /// it is on its way (the file or the decoding) or cannot be had.
    fn get(&self, assets: &mut Assets, path: &str) -> Option<AudioBuffer> {
        match self.0.borrow().get(path) {
            Some(Clip::Ready(buffer)) => return Some(buffer.clone()),
            Some(_) => return None,
            None => {}
        }
        let bytes = match assets.read(path) {
            Ok(bytes) => bytes,
            // Still downloading: asked again next time.
            Err(_) if crate::web_files::busy() => return None,
            Err(error) => {
                crate::web::log(&format!("Gravak audio: {path}: {error}"));
                self.0.borrow_mut().insert(path.to_owned(), Clip::Failed);
                return None;
            }
        };
        let context = context()?;
        self.0.borrow_mut().insert(path.to_owned(), Clip::Decoding);
        let clips = self.clone();
        let path = path.to_owned();
        wasm_bindgen_futures::spawn_local(async move {
            let data = js_sys::Uint8Array::from(bytes.as_slice()).buffer();
            let decoded = match context.decode_audio_data(&data) {
                Ok(promise) => JsFuture::from(promise).await.ok().and_then(|buffer| buffer.dyn_into().ok()),
                Err(_) => None,
            };
            let clip = match decoded {
                Some(buffer) => Clip::Ready(buffer),
                None => {
                    crate::web::log(&format!("Gravak audio: {path} could not be decoded"));
                    Clip::Failed
                }
            };
            clips.0.borrow_mut().insert(path, clip);
        });
        None
    }
}

/// Plays `buffer` once through `output`; the source it plays on.
fn play(buffer: &AudioBuffer, output: &GainNode, looping: bool) -> Option<AudioBufferSourceNode> {
    let context = context()?;
    let source = context.create_buffer_source().ok()?;
    source.set_buffer(Some(buffer));
    source.set_loop(looping);
    source.connect_with_audio_node(output).ok()?;
    source.start().ok()?;
    Some(source)
}

/// Stops `source` at `when` on the context's clock (0: now).
fn stop(source: &AudioBufferSourceNode, when: f64) {
    let scheduled: &web_sys::AudioScheduledSourceNode = source.as_ref();
    let _ = scheduled.stop_with_when(when);
}

pub struct MovementAudio {
    /// The page's files, for the clips asked later.
    assets: Assets,
    opened: bool,
    paths: [Option<String>; Action::COUNT],
    clips: Clips,
    /// The sound effects' volume.
    effects: Option<GainNode>,
    voices: HashMap<String, AudioBufferSourceNode>,
    last_action: Option<(Action, u64)>,
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

#[derive(Default)]
struct MusicPlayer {
    regions: Vec<MusicRegion>,
    /// The track asked for, and the one playing (its source and its gain).
    wanted: Option<String>,
    playing: Option<(String, AudioBufferSourceNode, GainNode)>,
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
            action_sound(&catalog, character.body_id, character.weapon_action, action).map(str::to_owned)
        });
        Ok(Self {
            assets: Assets::from_memory(std::iter::empty()),
            opened: false,
            paths,
            clips: Clips::default(),
            effects: None,
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
        if let Some(effects) = &self.effects {
            effects.gain().set_value(self.sound_volume);
        }
        if let Some((_, _, output)) = &self.music.playing {
            output.gain().set_value(self.music.volume);
        }
    }

    /// Plays the music of the region the hero stands in, changing track
    /// when the hero walks into another region.
    pub fn update_music(&mut self, assets: &mut Assets, map_id: u32, cell: [i32; 2]) {
        if !self.opened {
            return;
        }
        // The movement clips, asked until they are in.
        for path in self.paths.iter().flatten() {
            let _ = self.clips.get(assets, path);
        }
        let wanted = self
            .music
            .regions
            .iter()
            .find(|region| {
                let [x, y, width, height] = region.bounds;
                region.map_id == map_id && (x..x + width).contains(&cell[0]) && (y..y + height).contains(&cell[1])
            })
            .map(|region| region.track.clone());
        let playing = self.music.playing.as_ref().map(|(track, _, _)| track.clone());
        if wanted == playing {
            return;
        }
        if wanted != self.music.wanted {
            self.music.wanted = wanted.clone();
            // The old track fades and stops.
            if let Some((_, source, output)) = self.music.playing.take()
                && let Some(context) = context()
            {
                let now = context.current_time();
                let level = output.gain();
                let _ = level.set_value_at_time(level.value(), now);
                let _ = level.linear_ramp_to_value_at_time(0.0, now + MUSIC_FADE);
                stop(&source, now + MUSIC_FADE);
            }
        }
        let Some(track) = wanted else { return };
        // The new one rises once its clip is decoded.
        let Some(buffer) = self.clips.get(assets, &track) else { return };
        let Some(context) = context() else { return };
        let Some(output) = gain(&context, 0.0) else { return };
        let now = context.current_time();
        let _ = output.gain().set_value_at_time(0.0, now);
        let _ = output.gain().linear_ramp_to_value_at_time(self.music.volume, now + MUSIC_FADE);
        if let Some(source) = play(&buffer, &output, true) {
            self.music.playing = Some((track, source, output));
        }
    }

    pub fn open_device(&mut self) {
        let Some(context) = context() else { return };
        self.effects = gain(&context, self.sound_volume);
        self.opened = self.effects.is_some();
    }

    pub fn update(&mut self, player: &Player) {
        let action = (player.action, player.action_instance);
        if self.last_action == Some(action) {
            return;
        }
        self.last_action = Some(action);
        let Some(effects) = &self.effects else { return };
        let Some(path) = &self.paths[player.action.index()] else { return };
        let Some(buffer) = self.clips.get(&mut self.assets, path) else { return };
        if let Some(previous) = self.voices.remove(path) {
            stop(&previous, 0.0);
        }
        if let Some(voice) = play(&buffer, effects, false) {
            self.voices.insert(path.clone(), voice);
        }
    }

    /// Plays `path` once at the sound volume (a blow's hit); its clip is
    /// kept for the next time.
    pub fn play_once(&mut self, assets: &mut Assets, path: &str) {
        let Some(effects) = &self.effects else { return };
        if let Some(buffer) = self.clips.get(assets, path) {
            play(&buffer, effects, false);
        }
    }

    pub fn check(&self) -> Result<()> {
        Ok(())
    }
}

fn action_sound(catalog: &ActionSoundCatalog, body_id: u32, weapon_id: u32, action: Action) -> Option<&str> {
    let action_id = action.catalog_id();
    catalog
        .get(ActionSoundKey { body_id, weapon_id, action_id })
        .or_else(|| catalog.get(ActionSoundKey { body_id, weapon_id: 999, action_id }))
}

/// The login screens' sound: the theme looping softly behind them, and the
/// game's own clicks for buttons and page turns.
pub struct LoginAudio {
    assets: Assets,
    clips: Clips,
    music: Option<AudioBufferSourceNode>,
    effects: GainNode,
    music_volume: f32,
    /// Whether the theme has started (it starts once).
    started: Cell<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LoginSound {
    /// A button, a body, a profession or a dye chosen.
    Click,
    /// One screen giving way to another.
    Page,
}

const LOGIN_MUSIC: &str = "sound/music.mp3";

fn login_clip(sound: LoginSound) -> &'static str {
    match sound {
        LoginSound::Click => "sound/BtnClick.wav",
        LoginSound::Page => "sound/ChangePage.wav",
    }
}

impl LoginAudio {
    /// The page's sound for the login screens; `None` without Web Audio.
    /// Volumes are the Options window's (0..=100).
    pub fn start(_assets: &mut Assets, music_volume: u8, sound_volume: u8) -> Option<Self> {
        let context = context()?;
        let audio = Self {
            assets: Assets::from_memory(std::iter::empty()),
            clips: Clips::default(),
            music: None,
            effects: gain(&context, f32::from(sound_volume.min(100)) / 100.0)?,
            music_volume: f32::from(music_volume.min(100)) / 100.0,
            started: Cell::new(false),
        };
        Some(audio)
    }

    /// Asks for the clips and the theme until they are in; the theme rises
    /// over three seconds and loops.
    pub fn update(&mut self) {
        for sound in [LoginSound::Click, LoginSound::Page] {
            let _ = self.clips.get(&mut self.assets, login_clip(sound));
        }
        if self.started.get() {
            return;
        }
        let Some(theme) = self.clips.get(&mut self.assets, LOGIN_MUSIC) else { return };
        let Some(context) = context() else { return };
        let Some(output) = gain(&context, 0.0) else { return };
        let now = context.current_time();
        let _ = output.gain().set_value_at_time(0.0, now);
        let _ = output.gain().linear_ramp_to_value_at_time(self.music_volume * 0.8, now + 3.0);
        self.music = play(&theme, &output, true);
        self.started.set(true);
    }

    pub fn play(&self, sound: LoginSound) {
        let mut assets = Assets::from_memory(std::iter::empty());
        if let Some(clip) = self.clips.get(&mut assets, login_clip(sound)) {
            play(&clip, &self.effects, false);
        }
    }
}

impl Drop for LoginAudio {
    /// The theme stops with the screens, as natively when its sink goes.
    fn drop(&mut self) {
        if let Some(music) = &self.music {
            stop(music, 0.0);
        }
    }
}
