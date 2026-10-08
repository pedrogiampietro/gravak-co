//! The Gravak content pipeline.
//!
//! The original 5065 files (`itemtype.dat`, icons, ...) stay untouched in the
//! client installation. Our own content lives in the repository as readable
//! files (`content/items/*.toml`, `content/icons/*.png`) and the build merges
//! it over the originals into `content/build`, which the client
//! (`GRAVAK_CONTENT`) and the server (`ITEMTYPE_DATA`) read before the
//! installation.

pub mod accessories;
pub mod assets;
pub mod build;
pub mod c3mesh;
pub mod dds;
pub mod garments;
pub mod items;
pub mod itemtype;
pub mod maps;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;
