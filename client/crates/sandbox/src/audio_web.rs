//! The browser build's sound: silence (docs/web-spike.md). The same API as
//! `audio.rs`, whose rodio has no Web Audio backend wired in yet; `load`
//! and `start` fail, so the game runs with `audio: None`.
use crate::{
    assets::{Assets, Result},
    character::Character,
    movement::Player,
};

pub struct MovementAudio;

impl MovementAudio {
    pub fn load(_assets: &mut Assets, _character: &Character) -> Result<Self> {
        Err("no sound in the browser yet".into())
    }
    pub fn set_volumes(&mut self, _music: u8, _sound: u8) {}
    pub fn update_music(&mut self, _assets: &mut Assets, _map_id: u32, _cell: [i32; 2]) {}
    pub fn open_device(&mut self) {}
    pub fn update(&mut self, _player: &Player) {}
    pub fn play_once(&mut self, _assets: &mut Assets, _path: &str) {}
    pub fn check(&self) -> Result<()> {
        Ok(())
    }
}

pub struct LoginAudio;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LoginSound {
    Click,
    Page,
}

impl LoginAudio {
    pub fn start(_assets: &mut Assets, _music_volume: u8, _sound_volume: u8) -> Option<Self> {
        None
    }
    pub fn update(&mut self) {}
    pub fn play(&self, _sound: LoginSound) {}
}
