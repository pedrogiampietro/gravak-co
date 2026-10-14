//! The fonts `fontdb` finds on the machine. The browser lends none, so the
//! web build (docs/web-spike.md) carries KaTeX's sans (SIL OFL, see
//! `ui_assets/KaTeX_Main-Bold-LICENSE.txt`) and answers every family with
//! it.

pub fn load_system_fonts(database: &mut fontdb::Database) {
    #[cfg(not(target_arch = "wasm32"))]
    database.load_system_fonts();
    #[cfg(target_arch = "wasm32")]
    {
        const SANS: &[u8] = include_bytes!("ui_assets/KaTeX_SansSerif-Regular.ttf");
        database.load_font_data(SANS.to_vec());
        database.set_sans_serif_family("KaTeX_SansSerif");
        database.set_serif_family("KaTeX_SansSerif");
    }
}
