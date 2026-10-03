//! The Instagram set: the logo, a profile icon, posts and a story, painted
//! with the login screens' art (gold title, panels, embers), the game
//! world's views and the professions' 3D heroes.
//!
//! Run with the game installed at `GRAVAK_ASSETS` (default `R:/5065`):
//! `cargo test --release -p conquer-sandbox marketing -- --ignored`
//! The pictures land in `marketing/` at the repository root.

use crate::launcher_art as art;
use image::RgbaImage;

const POST: [u32; 2] = [1080, 1350];
const SQUARE: [u32; 2] = [1080, 1080];
const STORY: [u32; 2] = [1080, 1920];

/// (name, tagline, weapons, map, cell) for each profession's poster.
const CLASSES: [(&str, &str, &str, u32, [f32; 2]); 4] = [
    ("Trojan", "Mestre das lâminas gêmeas", "Lâminas · Espadas · Clavas", 1002, [430.0, 380.0]),
    ("Warrior", "O escudo inquebrável do reino", "Escudos · Lanças · Machados", 1002, [520.0, 350.0]),
    ("Archer", "A morte que vem de longe", "Arcos · Flechas", 1015, [717.0, 577.0]),
    ("Taoist", "Guardião do fogo e da água", "Magias · Espadas taoístas", 1011, [193.0, 266.0]),
];

fn output() -> std::path::PathBuf {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../marketing");
    std::fs::create_dir_all(&root).expect("marketing folder");
    root
}

/// Darkens the edges and the bands where text sits, so gold reads on any
/// picture: a vignette, and gradients at the top and the bottom.
fn frame_for_text(image: &mut RgbaImage, top: f32, bottom: f32) {
    let (w, h) = (image.width() as f32, image.height() as f32);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let (fx, fy) = (x as f32 / w, y as f32 / h);
        let vignette = 1.0 - (((fx - 0.5).powi(2) + (fy - 0.5).powi(2)).sqrt() - 0.35).max(0.0) * 1.3;
        let top_band = 1.0 - ((top - fy) / top).clamp(0.0, 1.0) * 0.75;
        let bottom_band = 1.0 - ((fy - (1.0 - bottom)) / bottom).clamp(0.0, 1.0) * 0.8;
        let k = (vignette * top_band * bottom_band).clamp(0.0, 1.0);
        for c in 0..3 {
            pixel.0[c] = (f32::from(pixel.0[c]) * k) as u8;
        }
    }
}

/// A thin gold frame inset from the picture's edge, with diamonds at the
/// middle of each side, like the login panels.
fn gold_border(image: &mut RgbaImage, inset: i32) {
    let (w, h) = (image.width() as i32, image.height() as i32);
    art::outline(image, [inset, inset, w - inset * 2, h - inset * 2], art::GOLD_DEEP);
    art::outline(image, [inset + 4, inset + 4, w - inset * 2 - 8, h - inset * 2 - 8], [176, 136, 66, 120]);
    for (x, y) in [(w / 2, inset), (w / 2, h - inset - 1), (inset, h / 2), (w - inset - 1, h / 2)] {
        art::diamond(image, x, y, 7, art::GOLD_BRIGHT);
    }
}

/// "GRAVAK" with its rule, centred on `centre_x`; `size` is the title's.
fn wordmark(image: &mut RgbaImage, size: f32, centre_x: i32, y: i32, subtitle: Option<&str>) {
    // The title's picture carries its glow around the letters: about
    // 1.55 sizes down is just under them.
    art::gold_title(image, "Gravak", size, centre_x, y);
    let under = y + (size * 1.55) as i32;
    if let Some(subtitle) = subtitle {
        art::label(image, subtitle, size * 0.2, centre_x, under, art::GOLD);
        art::ornament_rule(image, centre_x, under + (size * 0.34) as i32, (size * 2.6) as i32);
    } else {
        art::ornament_rule(image, centre_x, under, (size * 2.6) as i32);
    }
}

/// A soft oval shadow under a hero's feet.
fn hero_shadow(image: &mut RgbaImage, cx: i32, y: i32, radius: [i32; 2]) {
    for dy in -radius[1]..=radius[1] {
        for dx in -radius[0]..=radius[0] {
            let d = ((dx as f32 / radius[0] as f32).powi(2) + (dy as f32 / radius[1] as f32).powi(2)).sqrt();
            if d < 1.0 {
                art::blend(image, cx + dx, y + dy, [0, 0, 0, ((1.0 - d) * 170.0) as u8]);
            }
        }
    }
}

fn world(assets: &mut crate::assets::Assets, maps: &mut Vec<crate::map::Map>, map_id: u32, cell: [f32; 2], size: [u32; 2]) -> RgbaImage {
    if !maps.iter().any(|map| map.id == map_id) {
        maps.push(crate::map::Map::load(assets, map_id).expect("map"));
    }
    let map = maps.iter().find(|map| map.id == map_id).expect("map loaded");
    crate::login_scenes::view(assets, map, cell, size)
}

#[test]
#[ignore]
fn marketing() {
    let root = std::env::var("GRAVAK_ASSETS").unwrap_or_else(|_| "R:/5065".into());
    let mut assets = crate::assets::Assets::open(std::path::Path::new(&root)).expect("assets");
    art::set_frame_source(crate::original_panels::general_bg(&mut assets).expect("frame art"));
    let out = output();
    let mut maps = Vec::new();

    // 1. The logo, on our own art only (no game assets).
    let mut logo = art::backdrop(SQUARE);
    art::radial_glow(&mut logo, 540, 500, 420, [255, 190, 96, 40]);
    art::embers(&mut logo, 3.0);
    wordmark(&mut logo, 190.0, 540, 360, Some("CLASSIC  ·  5065"));
    gold_border(&mut logo, 36);
    logo.save(out.join("01-logo.png")).unwrap();

    // 2. The profile icon: a gold "G" in a ring, for the round crop.
    let mut icon = art::backdrop(SQUARE);
    art::radial_glow(&mut icon, 540, 540, 460, [255, 190, 96, 46]);
    for (radius, width, colour) in [(430.0, 14.0, art::GOLD_DEEP), (404.0, 4.0, art::GOLD_BRIGHT), (452.0, 3.0, [120, 92, 44, 255])] {
        for y in 0..1080 {
            for x in 0..1080 {
                let d = (((x - 540) * (x - 540) + (y - 540) * (y - 540)) as f32).sqrt();
                let coverage = (width / 2.0 + 0.5 - (d - radius).abs()).clamp(0.0, 1.0);
                if coverage > 0.0 {
                    art::blend(&mut icon, x, y, [colour[0], colour[1], colour[2], (coverage * 255.0) as u8]);
                }
            }
        }
    }
    art::gold_title(&mut icon, "G", 400.0, 540, 230);
    icon.save(out.join("02-profile-icon.png")).unwrap();

    // 3. One poster per profession.
    for (class, (name, tagline, weapons, map_id, cell)) in CLASSES.into_iter().enumerate() {
        let mut post = world(&mut assets, &mut maps, map_id, cell, POST);
        frame_for_text(&mut post, 0.32, 0.3);
        art::radial_glow(&mut post, 540, 760, 420, [255, 196, 110, 60]);
        let hero = crate::showcase::Hero::load(&mut assets, 3, class).expect("hero");
        let picture = hero.render([1000, 900], 760.0, 0.6, 0.35);
        hero_shadow(&mut post, 540, 300 + (900.0 * 0.93) as i32, [180, 24]);
        image::imageops::overlay(&mut post, &picture, 40, 300);
        art::gold_title(&mut post, name, 120.0, 540, 70);
        art::label(&mut post, tagline, 44.0, 540, 262, art::GOLD_BRIGHT);
        art::label(&mut post, weapons, 30.0, 540, 1150, art::GOLD);
        wordmark(&mut post, 60.0, 540, 1200, None);
        gold_border(&mut post, 24);
        post.save(out.join(format!("{:02}-{}.png", 3 + class, name.to_lowercase()))).unwrap();
    }

    // 4. The four professions together.
    let mut together = world(&mut assets, &mut maps, 1002, [430.0, 380.0], POST);
    frame_for_text(&mut together, 0.3, 0.3);
    art::gold_title(&mut together, "Escolha sua", 64.0, 540, 70);
    art::gold_title(&mut together, "Profissão", 96.0, 540, 150);
    for class in 0..4 {
        let hero = crate::showcase::Hero::load(&mut assets, if class % 2 == 0 { 3 } else { 2 }, class).expect("hero");
        let picture = hero.render([300, 640], 520.0, 0.6, 0.3);
        let x = 20 + class as i32 * 260 - 20;
        hero_shadow(&mut together, x + 150, 340 + (640.0 * 0.93) as i32, [90, 14]);
        image::imageops::overlay(&mut together, &picture, i64::from(x), 340);
        art::heading(&mut together, CLASSES[class].0, 34.0, x + 150, 1000, art::GOLD_BRIGHT, true);
    }
    wordmark(&mut together, 60.0, 540, 1180, None);
    gold_border(&mut together, 24);
    together.save(out.join("07-profissoes.png")).unwrap();

    // 5. The world.
    let mut post = world(&mut assets, &mut maps, 1015, [717.0, 577.0], POST);
    frame_for_text(&mut post, 0.35, 0.3);
    wordmark(&mut post, 150.0, 540, 90, Some("CLASSIC  ·  5065"));
    art::label(&mut post, "O mundo que você conhece.", 52.0, 540, 1080, art::GOLD_BRIGHT);
    art::label(&mut post, "Do jeito que você lembra.", 52.0, 540, 1150, art::GOLD);
    gold_border(&mut post, 24);
    post.save(out.join("08-o-mundo.png")).unwrap();

    // 6. A story teaser.
    let mut story = world(&mut assets, &mut maps, 1011, [193.0, 266.0], STORY);
    frame_for_text(&mut story, 0.3, 0.35);
    art::radial_glow(&mut story, 540, 1000, 520, [255, 196, 110, 64]);
    let hero = crate::showcase::Hero::load(&mut assets, 3, 0).expect("hero");
    let picture = hero.render([1040, 1100], 940.0, 0.6, 0.4);
    hero_shadow(&mut story, 540, 470 + (1100.0 * 0.93) as i32, [220, 28]);
    image::imageops::overlay(&mut story, &picture, 20, 470);
    wordmark(&mut story, 170.0, 540, 140, Some("CLASSIC  ·  5065"));
    art::gold_title(&mut story, "Em breve", 110.0, 540, 1600);
    art::embers(&mut story, 5.0);
    gold_border(&mut story, 28);
    story.save(out.join("09-story-em-breve.png")).unwrap();
}

/// A post's background: a world view, held well back so a panel of text
/// reads over it.
fn backdrop_post(assets: &mut crate::assets::Assets, maps: &mut Vec<crate::map::Map>, map_id: u32, cell: [f32; 2]) -> RgbaImage {
    let mut post = world(assets, maps, map_id, cell, POST);
    frame_for_text(&mut post, 0.3, 0.25);
    for pixel in post.pixels_mut() {
        for c in 0..3 {
            pixel.0[c] = (f32::from(pixel.0[c]) * 0.7) as u8;
        }
    }
    post
}

/// A list of (title, line) points on a framed glass panel: a gold diamond,
/// the title in the decorative face and its line beneath.
fn feature_panel(image: &mut RgbaImage, top: i32, points: &[(&str, &str)]) {
    let row = 112;
    let rect = [70, top, 940, 70 + row * points.len() as i32];
    art::panel(image, rect, None);
    for (index, (title, line)) in points.iter().enumerate() {
        let y = top + 48 + index as i32 * row;
        art::diamond(image, rect[0] + 64, y + 22, 9, art::GOLD_BRIGHT);
        art::heading(image, title, 38.0, rect[0] + 96, y, art::GOLD_BRIGHT, false);
        art::label_left(image, line, 27.0, rect[0] + 98, y + 50, [226, 232, 230, 255]);
    }
}

/// The series heading: a small gold kicker over a large title.
fn series_heading(image: &mut RgbaImage, kicker: &str, title: &str, size: f32) {
    art::label(image, kicker, 30.0, 540, 70, art::GOLD);
    art::ornament_rule(image, 540, 116, 300);
    art::gold_title(image, title, size, 540, 120);
}

/// A post's foot: the wordmark and the page of the carousel.
fn series_foot(image: &mut RgbaImage, page: usize, pages: usize) {
    wordmark(image, 52.0, 540, 1205, None);
    art::label(image, &format!("{page} / {pages}"), 24.0, 990, 1270, art::GOLD);
    gold_border(image, 24);
}

#[test]
#[ignore]
fn marketing_tech() {
    let root = std::env::var("GRAVAK_ASSETS").unwrap_or_else(|_| "R:/5065".into());
    let mut assets = crate::assets::Assets::open(std::path::Path::new(&root)).expect("assets");
    art::set_frame_source(crate::original_panels::general_bg(&mut assets).expect("frame art"));
    let out = output();
    let mut maps = Vec::new();
    let pages = 6;

    // 1. Cover.
    let mut post = backdrop_post(&mut assets, &mut maps, 1002, [430.0, 380.0]);
    art::radial_glow(&mut post, 540, 600, 520, [255, 190, 96, 56]);
    art::embers(&mut post, 2.0);
    art::label(&mut post, "POR DENTRO DO", 34.0, 540, 260, art::GOLD);
    art::gold_title(&mut post, "Gravak", 170.0, 540, 290);
    art::ornament_rule(&mut post, 540, 590, 480);
    art::gold_title(&mut post, "100% Rust", 110.0, 540, 640);
    art::label(&mut post, "Cliente e servidor construídos em Rust.", 38.0, 540, 900, art::GOLD_BRIGHT);
    art::label(&mut post, "Um clássico, reconstruído com tecnologia de hoje.", 30.0, 540, 956, [226, 232, 230, 255]);
    art::label(&mut post, "Arraste para o lado  ›", 28.0, 540, 1080, art::GOLD);
    series_foot(&mut post, 1, pages);
    post.save(out.join("10-tech-capa.png")).unwrap();

    // 2. The numbers.
    let mut post = backdrop_post(&mut assets, &mut maps, 1015, [717.0, 577.0]);
    series_heading(&mut post, "EM NÚMEROS", "O Projeto", 96.0);
    let stats = [
        ("88 mil", "linhas de Rust no total"),
        ("45 mil", "linhas no cliente, do login ao HUD"),
        ("42 mil", "linhas no servidor, de login a Guild War"),
        ("190", "testes automatizados no servidor"),
        ("5065", "compatível com o protocolo do cliente clássico"),
    ];
    let rect = [70, 320, 940, 60 + 148 * stats.len() as i32];
    art::panel(&mut post, rect, None);
    for (index, (number, line)) in stats.into_iter().enumerate() {
        let y = rect[1] + 44 + index as i32 * 148;
        art::gold_title(&mut post, number, 64.0, 260, y - 10);
        art::label_left(&mut post, line, 28.0, 430, y + 44, [226, 232, 230, 255]);
    }
    series_foot(&mut post, 2, pages);
    post.save(out.join("11-tech-numeros.png")).unwrap();

    // 3. The client.
    let mut post = backdrop_post(&mut assets, &mut maps, 1011, [193.0, 266.0]);
    series_heading(&mut post, "FEITO EM RUST", "O Cliente", 96.0);
    feature_panel(&mut post, 330, &[
        ("Gráficos pela GPU", "Vulkan, DirectX 12 ou Metal, via wgpu"),
        ("FPS sem limite", "VSync, 30 a 144 FPS ou ilimitado"),
        ("Mapa expansível", "Zoom e um pino para cada membro do time"),
        ("Música por região", "A trilha muda conforme você explora"),
        ("Login vivo", "O mundo e os heróis em 3D já na entrada"),
        ("Interface unificada", "Janelas no estilo clássico, polidas"),
    ]);
    series_foot(&mut post, 3, pages);
    post.save(out.join("12-tech-cliente.png")).unwrap();

    // 4. The server.
    let mut post = backdrop_post(&mut assets, &mut maps, 1000, [499.0, 650.0]);
    series_heading(&mut post, "FEITO EM RUST", "O Servidor", 96.0);
    feature_panel(&mut post, 330, &[
        ("Assíncrono", "Tokio: conexões tratadas em paralelo"),
        ("Autoritativo", "Combate, trade e drops decididos no servidor"),
        ("Economia auditada", "Cada item e cada moeda ficam registrados"),
        ("Seguro por padrão", "Rust evita as falhas de memória clássicas"),
        ("Testado", "190 testes automáticos a cada mudança"),
        ("Mundo no banco", "Mapas, NPCs e monstros vêm do banco de dados"),
    ]);
    series_foot(&mut post, 4, pages);
    post.save(out.join("13-tech-servidor.png")).unwrap();

    // 5. The game.
    let mut post = backdrop_post(&mut assets, &mut maps, 1020, [566.0, 565.0]);
    series_heading(&mut post, "O QUE TE ESPERA", "No Jogo", 96.0);
    feature_panel(&mut post, 330, &[
        ("PK clássico", "Nome piscando, red e black, prisão e guardas"),
        ("Raridade de itens", "Do comum ao mítico, com bônus aleatórios"),
        ("Gemas e soquetes", "Refinery, Artifact e composição +1 a +9"),
        ("Trade seguro", "Tudo validado no servidor, item por item"),
        ("Arena e Guild War", "Duelos, pólo, portões e cerco aos sábados"),
        ("Times de verdade", "Pinos no mapa, rosto e vida de cada membro"),
    ]);
    series_foot(&mut post, 5, pages);
    post.save(out.join("14-tech-jogo.png")).unwrap();

    // 6. Closing.
    let mut post = backdrop_post(&mut assets, &mut maps, 1002, [520.0, 350.0]);
    art::radial_glow(&mut post, 540, 640, 520, [255, 190, 96, 56]);
    art::embers(&mut post, 4.0);
    art::gold_title(&mut post, "O clássico", 110.0, 540, 230);
    art::gold_title(&mut post, "voltou", 110.0, 540, 400);
    art::ornament_rule(&mut post, 540, 610, 420);
    art::label(&mut post, "Siga para acompanhar o desenvolvimento.", 36.0, 540, 680, art::GOLD_BRIGHT);
    art::label(&mut post, "Ative as notificações e chame seu antigo time.", 30.0, 540, 736, [226, 232, 230, 255]);
    art::gold_title(&mut post, "Em breve", 90.0, 540, 860);
    series_foot(&mut post, 6, pages);
    post.save(out.join("15-tech-em-breve.png")).unwrap();
}
