// Source: @digitalm1nd on x.com / _dek on Discord — Preservation Conquer project — https://discord.gg/CvKPXEHYRY
mod action;
mod assets;
mod audio;
mod character;
mod labels;
mod map;
mod minimap;
mod motion_math;
mod movement;
mod network;
mod renderer;
mod validation;
use action::Action;
use assets::{Assets, Result};
use motion_math::direction_towards;
use network::{NetworkConfig, NetworkEvent, NetworkHandle, RemoteActor};
use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::Arc,
    time::Instant,
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};
struct App {
    assets: Assets,
    map: map::Map,
    character: character::Character,
    player: movement::Player,
    audio: Option<audio::MovementAudio>,
    window: Option<Arc<Window>>,
    renderer: Option<renderer::Renderer>,
    cursor: [f32; 2],
    started: Instant,
    last: Instant,
    error: Option<String>,
    modifiers: winit::keyboard::ModifiersState,
    held: bool,
    queued_jump: Option<[i32; 2]>,
    network: Option<NetworkHandle>,
    network_character_id: Option<u32>,
    confirmed_network_cell: Option<[i32; 2]>,
    pending_network_steps: VecDeque<[i32; 2]>,
    remote_actors: HashMap<u32, RemoteActor>,
    network_status: String,
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| -> Result<_> {
            let window = Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title("gravak-co — offline Twin City sandbox")
                        .with_inner_size(winit::dpi::PhysicalSize::new(1366, 768)),
                )?,
            );
            let renderer = pollster::block_on(renderer::Renderer::new(window.clone()))?;
            Ok((window, renderer))
        })();
        match result {
            Ok((window, renderer)) => {
                self.window = Some(window);
                self.renderer = Some(renderer);
                self.last = Instant::now();
            }
            Err(e) => {
                self.error = Some(e.to_string());
                event_loop.exit();
            }
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height)
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = [position.x as f32, position.y as f32]
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::Focused(false) => {
                self.held = false;
                self.queued_jump = None;
                self.player.stop();
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                if self.renderer.as_mut().is_some_and(|renderer| {
                    renderer.minimap_pointer(self.cursor, state == ElementState::Pressed)
                }) {
                    self.held = false;
                    return;
                }
                self.held = state == ElementState::Pressed;
                if self.held {
                    self.move_cursor();
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Escape) => event_loop.exit(),
                    PhysicalKey::Code(KeyCode::Space) => self.player.stop(),
                    PhysicalKey::Code(KeyCode::Slash) => self.player.running = !self.player.running,
                    _ => {}
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let previous_cell = self.player.cell();
                self.player
                    .tick(now.duration_since(self.last).as_secs_f32());
                self.last = now;
                self.send_network_step(previous_cell);
                if !self.player.moving()
                    && let Some(target) = self.queued_jump.take()
                {
                    self.player
                        .jump(&self.map, target, self.character.duration(Action::Jump));
                } else if self.held && !self.player.moving() {
                    self.move_cursor();
                }
                if let Some(audio) = &mut self.audio {
                    audio.update(&self.player);
                }
                if let Some(renderer) = &mut self.renderer
                    && let Err(error) = renderer.draw(
                        &mut self.assets,
                        &self.map,
                        &self.character,
                        &self.player,
                        &self.remote_actors,
                        self.started.elapsed().as_secs_f32(),
                    )
                {
                    self.error = Some(error.to_string());
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let events = self
            .network
            .as_ref()
            .map(NetworkHandle::drain)
            .unwrap_or_default();
        for event in events {
            self.handle_network_event(event);
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
impl App {
    fn move_cursor(&mut self) {
        if let Some(renderer) = &self.renderer {
            let camera = renderer.camera(&self.map, &self.player);
            let target = self
                .map
                .unproject([self.cursor[0] + camera[0], self.cursor[1] + camera[1]])
                .map(|v| v.round() as i32);
            if self.modifiers.control_key() {
                self.player.stop();
                if self.player.moving() {
                    self.queued_jump = Some(target);
                    return;
                }
                self.player
                    .jump(&self.map, target, self.character.duration(Action::Jump));
            } else if self.modifiers.shift_key() {
                self.player.face(target);
            } else {
                self.player.go(&self.map, target);
            }
        }
    }

    fn handle_network_event(&mut self, event: NetworkEvent) {
        match event {
            NetworkEvent::Status(status) => self.network_status = status,
            NetworkEvent::Ready(ready) => {
                self.player.position = ready.position.map(|value| value as f32);
                self.character.name = ready.character_name;
                self.confirmed_network_cell = Some(ready.position);
                self.pending_network_steps.clear();
                self.network_status =
                    format!("online: char {} map {}", ready.character_id, ready.map_id);
            }
            NetworkEvent::WalkConfirmed {
                character_id,
                direction,
            } => {
                if let Some(position) = self.pending_network_steps.pop_front() {
                    self.confirmed_network_cell = Some(position);
                }
                self.network_status =
                    format!("online: walk confirmed {character_id} dir {direction}");
            }
            NetworkEvent::MovementRejected(message) => {
                self.pending_network_steps.clear();
                if let Some(position) = self.confirmed_network_cell {
                    self.player.position = position.map(|value| value as f32);
                }
                self.player.stop();
                self.network_status = format!("online: movement rejected ({message})");
            }
            NetworkEvent::Reposition {
                map_id,
                position,
                direction,
            } => {
                self.pending_network_steps.clear();
                self.confirmed_network_cell = Some(position);
                self.player.position = position.map(|value| value as f32);
                self.player.stop();
                self.network_status = format!("online: repositioned map {map_id} dir {direction}");
            }
            NetworkEvent::PlayerSpawn(actor) | NetworkEvent::NpcSpawn(actor) => {
                let kind = if actor.npc { "npc" } else { "player" };
                let id = actor.id;
                self.remote_actors.insert(id, actor);
                if let Some(actor) = self.remote_actors.get(&id) {
                    self.network_status = format!(
                        "online: {} {} mesh {} ({} actors)",
                        kind,
                        actor.name,
                        actor.mesh,
                        self.remote_actors.len()
                    );
                }
            }
            NetworkEvent::Error(error) => self.network_status = format!("network error: {error}"),
        }
        if let Some(window) = &self.window {
            window.set_title(&format!("gravak-co — {}", self.network_status));
        }
    }
    fn send_network_step(&mut self, previous_cell: [i32; 2]) {
        let Some(network) = &self.network else {
            return;
        };
        let Some(character_id) = self.network_character_id else {
            return;
        };
        let current_cell = self.player.cell();
        let previous_predicted = self
            .pending_network_steps
            .back()
            .copied()
            .or(self.confirmed_network_cell)
            .unwrap_or(previous_cell);
        if current_cell == previous_predicted {
            return;
        }
        let direction = direction_towards(
            previous_predicted.map(|value| value as f32),
            current_cell.map(|value| value as f32),
        );
        network.send_walk(character_id, direction);
        self.pending_network_steps.push_back(current_cell);
        self.network_status = format!("online: sending walk dir {direction}");
    }
}
fn run() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let mut root = None;
    let mut map_id = None;
    let mut body =
        sandbox_data::ini::SectionCatalog::parse(include_bytes!("../../../character.ini"))
            .section("Character")
            .and_then(|section| section.first("Body"))
            .unwrap_or("3")
            .parse::<u32>()?;
    let mut spawn = None;
    let mut check = false;
    let mut online = false;
    let mut auth_address = "127.0.0.1:9958".to_owned();
    let mut username = "test1".to_owned();
    let mut password = "123456".to_owned();
    let mut realm = "CoEmu".to_owned();
    let mut character_name = None;
    while let Some(arg) = args.next() {
        match arg.to_str().ok_or("Invalid argument encoding")? {
            "--assets" => {
                root = Some(PathBuf::from(
                    args.next().ok_or("--assets requires a directory")?,
                ))
            }
            "--map" => {
                map_id = Some(
                    args.next()
                        .ok_or("--map requires an ID")?
                        .to_str()
                        .ok_or("Invalid map")?
                        .parse()?,
                )
            }
            "--body" => {
                body = args
                    .next()
                    .ok_or("--body requires 1..4")?
                    .to_str()
                    .ok_or("Invalid body")?
                    .parse()?
            }
            "--spawn" => {
                let x = args
                    .next()
                    .ok_or("--spawn requires X Y")?
                    .to_str()
                    .ok_or("Invalid X")?
                    .parse()?;
                let y = args
                    .next()
                    .ok_or("--spawn requires X Y")?
                    .to_str()
                    .ok_or("Invalid Y")?
                    .parse()?;
                spawn = Some([x, y]);
            }
            "--check-assets" => check = true,
            "--online" => online = true,
            "--auth" => {
                auth_address = args
                    .next()
                    .ok_or("--auth requires an address")?
                    .to_str()
                    .ok_or("Invalid auth address")?
                    .to_owned();
                online = true;
            }
            "--username" => {
                username = args
                    .next()
                    .ok_or("--username requires a value")?
                    .to_str()
                    .ok_or("Invalid username")?
                    .to_owned();
            }
            "--password" => {
                password = args
                    .next()
                    .ok_or("--password requires a value")?
                    .to_str()
                    .ok_or("Invalid password")?
                    .to_owned();
            }
            "--realm" => {
                realm = args
                    .next()
                    .ok_or("--realm requires a value")?
                    .to_str()
                    .ok_or("Invalid realm")?
                    .to_owned();
            }
            "--character" => {
                character_name = Some(
                    args.next()
                        .ok_or("--character requires a name")?
                        .to_str()
                        .ok_or("Invalid character name")?
                        .to_owned(),
                );
            }
            "--help" | "-h" => {
                println!(
                    "You must supply your own Conquer Online 5065 installation.\ngravak-co --assets <installation> [--online|--auth HOST:PORT] [--username USER] [--password PASS] [--realm REALM] [--character NAME] [--map ID] [--spawn X Y] [--body 1..4] [--check-assets]\nOffline by default. In online mode the client authenticates and loads the server character/map. Click to move; Space stops; Escape exits. No bundled assets."
                );
                return Ok(());
            }
            _ => return Err("Unknown argument; use --help".into()),
        }
    }
    let mut network = None;
    let mut server_ready = None;
    if online {
        let handle = NetworkHandle::start(NetworkConfig {
            auth_address,
            username,
            password,
            realm,
            character_name,
        });
        let ready = handle.wait_ready()?;
        if map_id.is_none() {
            map_id = Some(ready.map_id);
        }
        server_ready = Some(ready);
        network = Some(handle);
    }
    let map_id = map_id.unwrap_or(1002);
    let mut assets = Assets::open(
        &root.ok_or("You must supply your own 5065 installation: --assets <directory>")?,
    )?;
    let map = map::Map::load(&mut assets, map_id)?;
    let mut character = character::Character::load(&mut assets, body)?;
    if let Some(ready) = &server_ready {
        character.name = ready.character_name.clone();
    }
    let spawn = spawn
        .or_else(|| server_ready.as_ref().map(|ready| ready.position))
        .or(if map_id == 1002 {
            Some([430, 380])
        } else {
            None
        });
    let player = movement::Player::new(&map, spawn)?;
    let mut audio = match audio::MovementAudio::load(&mut assets, &character) {
        Ok(audio) => Some(audio),
        Err(error) if check => return Err(error),
        Err(error) => {
            eprintln!("Audio unavailable: {error}");
            None
        }
    };
    if check {
        if let Some(audio) = &audio {
            audio.check()?;
        }
        for part in &character.parts {
            assets.image(&part.texture)?;
        }
        validation::movement(&map, &character, player.cell())?;
        if let Some(minimap) = minimap::Minimap::load(&mut assets, map.id)? {
            let center = map.project(player.position);
            let quads = minimap.quads(
                &map,
                player.position,
                [center[0] - 683.0, center[1] - 384.0],
                [1366.0, 768.0],
            );
            if quads.len() != 4 {
                return Err("Minimap crop/hero/control check failed".into());
            }
            println!("Minimap OK: original assets, projected hero, crop and controls");
        }
        println!(
            "Assets OK: map {map_id}, {}x{}, {} sprites; character dek at {:?}",
            map.data.width,
            map.data.height,
            map.sprites.len(),
            player.cell()
        );
        return Ok(());
    }
    if let Some(audio) = &mut audio {
        audio.open_device();
    }
    let mut app = App {
        assets,
        map,
        character,
        player,
        audio,
        window: None,
        renderer: None,
        cursor: [0.0; 2],
        started: Instant::now(),
        last: Instant::now(),
        error: None,
        modifiers: Default::default(),
        held: false,
        queued_jump: None,
        network,
        network_character_id: server_ready.as_ref().map(|ready| ready.character_id),
        confirmed_network_cell: server_ready.as_ref().map(|ready| ready.position),
        pending_network_steps: VecDeque::new(),
        remote_actors: HashMap::new(),
        network_status: if online {
            "online".into()
        } else {
            "offline".into()
        },
    };
    EventLoop::new()?.run_app(&mut app)?;
    if let Some(error) = app.error {
        return Err(error.into());
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Sandbox: {error}");
        std::process::exit(1);
    }
}
