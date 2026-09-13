//! The app shell: the window, the frame loop, and the run control.
//!
//! The simulation and the renderer are decoupled on purpose. The renderer never
//! gates the simulation, and the simulation never gates the renderer: the frame
//! loop decides how many fixed steps to run from the speed control and a frame
//! budget, and every Episode draws on a stream derived from
//! `(seed, generation, member)`. That is why a slow frame cannot change the
//! outcome of a run.
//!
//! The member being watched steps on the main thread — so the Arena shows the
//! Episode currently being evaluated — while the rest of the Population
//! evaluates on a worker pool in the background. Episodes are independent, so
//! the run is identical either way (ADR 0005).

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use sim::{
    config, default_workers, EpisodeOutcome, Generation, GenomeFile, Run, RunOptions, World,
};

use crate::gpu::Gpu;
use crate::painter::{Painter, Transform};
use crate::renderer::Renderer;
use crate::scene::{self, ArenaView};
use crate::{panels, theme, ui};

/// How long a frame may spend stepping the watched Episode, in seconds. This is
/// what keeps the frame rate steady while the run goes as fast as it can.
const SIM_BUDGET: f64 = 0.008;
/// The window the measured rate is averaged over.
const RATE_WINDOW: f64 = 1.0;
/// How long a Generation-complete banner stays up.
const BANNER_SECONDS: f64 = 1.2;
/// Where Save writes and Load reads.
const SAVES_DIR: &str = "saves";

pub fn run() {
    let event_loop = match EventLoop::new() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            eprintln!("NeuroArena cannot open a window: {error}");
            std::process::exit(1);
        }
    };
    let mut app = App::new();
    if let Err(error) = event_loop.run_app(&mut app) {
        eprintln!("NeuroArena stopped: {error}");
        std::process::exit(1);
    }
}

struct App {
    gpu: Option<Gpu>,
    window: Option<Arc<Window>>,
    surface: Option<wgpu::Surface<'static>>,
    surface_format: wgpu::TextureFormat,
    config: Option<wgpu::SurfaceConfiguration>,
    renderer: Option<Renderer>,
    painter: Painter,

    seed: u32,
    run: Run,
    workers: usize,
    best_member_hint: usize,

    generation: Option<Arc<Generation>>,
    watched: Option<World>,
    watched_member: usize,
    worker: Option<Receiver<Vec<Option<EpisodeOutcome>>>>,
    outcomes: Vec<Option<EpisodeOutcome>>,

    controls: ui::Controls,
    hot: Option<ui::Hit>,
    cursor: Option<PhysicalPosition<f64>>,
    dragging_slider: bool,
    message: String,
    message_is_error: bool,
    banner: Option<(String, f64)>,

    loaded: Option<(PathBuf, GenomeFile)>,
    load_cursor: usize,

    measured_rate: f64,
    rate_window: f64,
    rate_steps: u64,
    last_frame: Instant,
    /// True only once the surface has been configured for presentation. macOS
    /// can ask the window to draw before setup finishes, and a draw into an
    /// unconfigured surface is a validation error that aborts the process
    /// inside the system's draw callback, where unwinding is not allowed.
    ready: bool,
}

impl App {
    fn new() -> Self {
        let seed = fresh_seed();
        let workers = default_workers();
        let controls = ui::Controls {
            seed_text: seed.to_string(),
            ..ui::Controls::default()
        };
        Self {
            gpu: None,
            window: None,
            surface: None,
            surface_format: wgpu::TextureFormat::Bgra8UnormSrgb,
            config: None,
            renderer: None,
            painter: Painter::new(),
            seed,
            run: Run::with_options(seed, RunOptions::new(config::neat::POP_SIZE, workers)),
            workers,
            best_member_hint: 0,
            generation: None,
            watched: None,
            watched_member: 0,
            worker: None,
            outcomes: Vec::new(),
            controls,
            hot: None,
            cursor: None,
            dragging_slider: false,
            message: format!("evolving from seed {seed} · {workers} workers"),
            message_is_error: false,
            banner: None,
            loaded: None,
            load_cursor: 0,
            measured_rate: 0.0,
            rate_window: 0.0,
            rate_steps: 0,
            last_frame: Instant::now(),
            ready: false,
        }
    }

    // ---- run control -----------------------------------------------------

    /// Start again from Generation 1: the same seed replays the run, a new seed
    /// explores another lineage. `watching` replays a loaded Genome instead.
    fn restart(&mut self, seed: u32, watching: Option<(PathBuf, GenomeFile)>) {
        self.seed = seed;
        self.controls.seed_text = seed.to_string();
        self.controls.watching = watching.is_some();
        let options = RunOptions::new(config::neat::POP_SIZE, self.workers);
        self.run = match &watching {
            Some((_, file)) => Run::watch(seed, &file.genome, options),
            None => Run::with_options(seed, options),
        };
        self.reset_progress();
        self.loaded = watching;
        self.message = match &self.loaded {
            Some((path, file)) => format!(
                "watching {} · evolved to Generation {} · fitness {:.1}",
                path.display(),
                file.generation,
                file.fitness
            ),
            None => format!("evolving from seed {seed}"),
        };
        self.message_is_error = false;
    }

    fn reset_progress(&mut self) {
        self.generation = None;
        self.watched = None;
        self.worker = None;
        self.outcomes.clear();
        self.banner = None;
        self.best_member_hint = 0;
        self.rate_steps = 0;
        self.rate_window = 0.0;
    }

    /// Leave watch mode and evolve again, starting from the loaded Genome.
    fn evolve_from_loaded(&mut self) {
        let Some((path, file)) = self.loaded.clone() else {
            self.message = "load a Genome first, then Evolve".to_string();
            self.message_is_error = false;
            return;
        };
        let options = RunOptions::new(config::neat::POP_SIZE, self.workers);
        self.run = Run::resume_from(self.seed, &file.genome, options);
        self.reset_progress();
        self.controls.watching = false;
        self.message = format!(
            "evolving from {} · {} nodes · {} connections",
            path.display(),
            file.genome.node_count(),
            file.genome.enabled_connection_count()
        );
        self.message_is_error = false;
    }

    fn save_best(&mut self) {
        let Some(file) = self.run.best_file() else {
            self.message = "no Generation has finished yet, nothing to save".to_string();
            self.message_is_error = false;
            return;
        };
        let path = PathBuf::from(SAVES_DIR).join(file.file_name());
        match file.save(&path) {
            Ok(()) => {
                self.message = format!(
                    "saved {} · Generation {} · fitness {:.1}",
                    path.display(),
                    file.generation,
                    file.fitness
                );
                self.message_is_error = false;
            }
            Err(error) => {
                self.message = error.to_string();
                self.message_is_error = true;
            }
        }
    }

    /// Load the next save file, and replay it with breeding switched off. The
    /// seed field chooses the Asteroid fields the Genome is watched against.
    fn load_next(&mut self) {
        let mut files = save_files();
        if files.is_empty() {
            self.message = format!("no save files in {SAVES_DIR}/ — press Save first");
            self.message_is_error = true;
            return;
        }
        files.sort();
        let path = files[self.load_cursor % files.len()].clone();
        self.load_cursor = self.load_cursor.wrapping_add(1);
        match GenomeFile::load(&path) {
            Ok(file) => {
                let seed = self.controls.seed_value().unwrap_or(self.seed);
                let generation = file.generation;
                self.restart(seed, Some((path.clone(), file)));
                self.message = format!(
                    "watching {} · replaying its Generation {generation} · seed {seed}",
                    path.display()
                );
                self.message_is_error = false;
            }
            Err(error) => {
                self.message = error.to_string();
                self.message_is_error = true;
            }
        }
    }

    // ---- the frame -------------------------------------------------------

    /// Freeze the current Generation for evaluation and farm out every member
    /// but the one being watched.
    fn start_generation(&mut self) {
        let generation = Arc::new(self.run.begin_generation());
        if generation.is_empty() {
            return;
        }
        self.watched_member = self.best_member_hint.min(generation.len() - 1);
        self.watched = Some(generation.world(self.watched_member));
        self.outcomes = vec![None; generation.len()];

        let (sender, receiver) = std::sync::mpsc::channel();
        let workers = self.workers;
        let watched = self.watched_member;
        let handle = generation.clone();
        std::thread::Builder::new()
            .name("neuroarena-workers".to_string())
            .spawn(move || {
                let _ = sender.send(handle.evaluate_except(workers, &[watched]));
            })
            .expect("the worker pool thread starts");
        self.generation = Some(generation);
        self.worker = Some(receiver);
    }

    /// Advance the run for one frame.
    fn advance(&mut self, dt: f64) {
        if self.controls.paused {
            return;
        }
        if self.generation.is_none() {
            self.start_generation();
        }
        if self.generation.is_none() {
            return;
        }

        // 1. Step the watched Episode inside the frame's simulation budget.
        let speed = self.controls.speed;
        let unbounded = ui::Controls::is_unbounded(speed);
        let wanted = if unbounded {
            u64::MAX
        } else {
            (speed * dt / sim::DT).floor().max(0.0) as u64
        };
        let started = Instant::now();
        let mut stepped = 0u64;
        if let Some(world) = self.watched.as_mut() {
            while !world.done && stepped < wanted {
                world.step_fixed();
                stepped += 1;
                if stepped & 255 == 0 && started.elapsed().as_secs_f64() >= SIM_BUDGET {
                    break;
                }
            }
        }
        self.rate_steps += stepped;
        crate::gpu::take_error();

        // 2. Collect the rest of the Population.
        if let Some(receiver) = &self.worker {
            match receiver.try_recv() {
                Ok(results) => {
                    self.rate_steps += results
                        .iter()
                        .flatten()
                        .map(|outcome| outcome.steps)
                        .sum::<u64>();
                    for (index, outcome) in results.into_iter().enumerate() {
                        if let Some(outcome) = outcome {
                            self.outcomes[index] = Some(outcome);
                        }
                    }
                    self.worker = None;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    crate::gpu::note_error("the worker pool stopped; restart the run");
                    self.worker = None;
                }
            }
        }

        // 3. Close the Generation once the watched Episode and the pool agree.
        let watched_done = self
            .watched
            .as_ref()
            .map(|world| world.done)
            .unwrap_or(false);
        let pool_done = self.worker.is_none()
            && self
                .outcomes
                .iter()
                .enumerate()
                .all(|(index, outcome)| outcome.is_some() || index == self.watched_member);
        if !(watched_done && pool_done) {
            return;
        }
        self.finish_generation();
    }

    fn finish_generation(&mut self) {
        let Some(generation) = self.generation.take() else {
            return;
        };
        if let Some(world) = self.watched.take() {
            self.outcomes[self.watched_member] =
                Some(generation.outcome(self.watched_member, &world));
        }
        let outcomes: Option<Vec<EpisodeOutcome>> =
            self.outcomes.iter_mut().map(|slot| slot.take()).collect();
        let outcomes = match outcomes {
            Some(outcomes) => outcomes,
            None => {
                crate::gpu::note_error("a Generation finished with members missing");
                return;
            }
        };
        let report = self.run.complete_generation(outcomes);
        self.best_member_hint = report.best_member;
        self.banner = Some((
            format!(
                "Generation {} · best fitness {:.1} · alive {:.1}s · Wave {} · {} Species",
                report.generation,
                report.best,
                report.gate.best_alive_time,
                report.gate.best_wave,
                report.species_count
            ),
            BANNER_SECONDS,
        ));
        self.start_generation();
    }

    fn update_rate(&mut self, dt: f64) {
        self.rate_window += dt;
        if self.rate_window >= RATE_WINDOW {
            self.measured_rate = (self.rate_steps as f64 * sim::DT) / self.rate_window;
            self.rate_steps = 0;
            self.rate_window = 0.0;
        }
    }

    // ---- input -----------------------------------------------------------

    fn layout(&self) -> Option<ui::Layout> {
        let config = self.config.as_ref()?;
        let dpr = self.dpr();
        Some(ui::Layout::new(
            config.width as f32 / dpr,
            config.height as f32 / dpr,
        ))
    }

    fn dpr(&self) -> f32 {
        self.window
            .as_ref()
            .map(|window| window.scale_factor() as f32)
            .unwrap_or(1.0)
    }

    fn on_move(&mut self, position: PhysicalPosition<f64>) {
        self.cursor = Some(position);
        let dpr = self.dpr();
        let point = [(position.x as f32) / dpr, (position.y as f32) / dpr];
        let layout = self.layout();
        self.hot = layout.as_ref().and_then(|layout| layout.hit(point));
        if self.dragging_slider {
            if let Some(layout) = layout {
                self.set_speed_from_slider(&layout, point);
            }
        }
    }

    fn on_press(&mut self) {
        let Some(position) = self.cursor else {
            return;
        };
        let dpr = self.dpr();
        let point = [(position.x as f32) / dpr, (position.y as f32) / dpr];
        let Some(layout) = self.layout() else {
            return;
        };
        let Some(hit) = layout.hit(point) else {
            self.controls.seed_editing = false;
            return;
        };
        match hit {
            ui::Hit::SpeedSlider => {
                self.dragging_slider = true;
                self.set_speed_from_slider(&layout, point);
            }
            ui::Hit::SeedField => {
                self.controls.seed_editing = true;
                if self.controls.seed_text.is_empty() {
                    self.controls.seed_text = self.seed.to_string();
                }
            }
            ui::Hit::Pause => self.controls.paused = !self.controls.paused,
            ui::Hit::Rays => self.controls.rays = !self.controls.rays,
            ui::Hit::Restart => {
                let seed = self.seed;
                let watching = self.loaded.clone();
                self.restart(seed, watching);
            }
            ui::Hit::NewSeed => {
                let seed = fresh_seed();
                self.restart(seed, None);
            }
            ui::Hit::Save => self.save_best(),
            ui::Hit::Load => self.load_next(),
            ui::Hit::Evolve => self.evolve_from_loaded(),
        }
    }

    fn set_speed_from_slider(&mut self, layout: &ui::Layout, point: [f32; 2]) {
        let track = layout.speed_slider;
        let t = ((point[0] - track.x) / track.w.max(1.0)).clamp(0.0, 1.0);
        self.controls.speed = ui::Controls::speed_from_slider(t);
    }

    fn on_key(&mut self, key: &Key, state: ElementState) {
        if state != ElementState::Pressed {
            return;
        }
        match key {
            Key::Named(NamedKey::Space) => self.controls.paused = !self.controls.paused,
            Key::Named(NamedKey::Escape) => self.controls.seed_editing = false,
            Key::Named(NamedKey::Backspace) => {
                if self.controls.seed_editing {
                    self.controls.pop_seed_char();
                }
            }
            Key::Named(NamedKey::Enter) => {
                self.controls.seed_editing = false;
                match self
                    .controls
                    .seed_value()
                    .or_else(|| self.controls.seed_text.parse().ok())
                {
                    Some(seed) => self.restart(seed, None),
                    _ => {
                        self.message = "the seed field must hold a non-negative number".to_string();
                        self.message_is_error = true;
                    }
                }
            }
            Key::Character(text) => {
                let mut typed = false;
                for character in text.chars() {
                    if character.is_ascii_digit() {
                        self.controls.seed_editing = true;
                        self.controls.push_seed_char(character);
                        typed = true;
                    }
                }
                if typed {
                    return;
                }
                match text.to_lowercase().as_str() {
                    "r" => self.controls.rays = !self.controls.rays,
                    "s" => self.save_best(),
                    "l" => self.load_next(),
                    "e" => self.evolve_from_loaded(),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    // ---- drawing ---------------------------------------------------------

    /// Fill the Painter for this frame and return the physical extent.
    fn build_frame(&mut self) -> [f32; 2] {
        let dpr = self.dpr();
        let (physical, logical): ([f32; 2], [f32; 2]) = match self.config.as_ref() {
            Some(config) => (
                [config.width as f32, config.height as f32],
                [config.width as f32 / dpr, config.height as f32 / dpr],
            ),
            None => ([1.0, 1.0], [1.0, 1.0]),
        };
        let layout = ui::Layout::new(logical[0], logical[1]);

        let painter = &mut self.painter;
        painter.clear();
        painter.set_transform(Transform::new(dpr, [0.0, 0.0]));
        painter.rect(0.0, 0.0, logical[0], logical[1], theme::color::APP_BG);

        // The Arena keeps its 960×600 shape and scales into whatever space the
        // sidebar leaves; the panels stay at 1:1 and stay legible (ADR 0006).
        if let Some(world) = self.watched.as_ref() {
            let fit = (layout.arena.w / theme::layout::ARENA_WIDTH)
                .min(layout.arena.h / theme::layout::ARENA_HEIGHT)
                .max(0.05);
            let inner = [
                theme::layout::ARENA_WIDTH * fit,
                theme::layout::ARENA_HEIGHT * fit,
            ];
            let origin = [
                (layout.arena.x + (layout.arena.w - inner[0]) * 0.5) * dpr,
                (layout.arena.y + (layout.arena.h - inner[1]) * 0.5) * dpr,
            ];
            scene::draw_arena(
                painter,
                world,
                ArenaView {
                    origin,
                    scale: fit * dpr,
                },
                self.controls.rays,
            );
        }
        painter.set_transform(Transform::new(dpr, [0.0, 0.0]));

        let world = self.watched.as_ref();
        let info = panels::HudInfo {
            seed: self.seed,
            generation: self.run.generation(),
            member: self.watched_member,
            population: self.run.population().genomes.len(),
            episode_time: world.map(|world| world.time).unwrap_or(0.0),
            wave: world.map(|world| world.wave).unwrap_or(0),
            fitness: world.map(|world| world.agent.fitness).unwrap_or(0.0),
            competence: sim::Competence {
                alive_time: world
                    .map(|world| world.agent.stats.alive_time)
                    .unwrap_or(0.0),
                wave: world.map(|world| world.wave).unwrap_or(0),
                asteroids: world
                    .map(|world| world.agent.stats.asteroid_points)
                    .unwrap_or(0.0),
            },
            gate: self.run.gate(),
            species: self.run.population().species.len(),
            speed: self.controls.speed,
            measured_rate: self.measured_rate,
            watching: self.run.is_watching(),
        };
        panels::draw_hud(painter, layout.hud, &info);
        panels::draw_chart(painter, layout.chart, self.run.history());
        if let Some(world) = world {
            if let Some(network) = world.agent.network() {
                let index = self
                    .watched_member
                    .min(self.run.population().genomes.len().saturating_sub(1));
                if let Some(genome) = self.run.population().genomes.get(index) {
                    panels::draw_network(painter, layout.network, genome, network);
                }
            }
        }
        panels::draw_controls(painter, &layout, &self.controls, self.hot);
        let (status, is_error) = match crate::gpu::take_error() {
            Some(error) => (error, true),
            None => (self.message.clone(), self.message_is_error),
        };
        panels::draw_status(painter, layout.status, &status, is_error);
        if let Some((text, _)) = self.banner.as_ref() {
            panels::draw_banner(painter, layout.arena, &[(text.clone(), theme::color::TEXT)]);
        }
        physical
    }

    fn draw(&mut self) {
        if !self.ready {
            return;
        }
        let physical = self.build_frame();

        let (Some(gpu), Some(renderer)) = (self.gpu.as_ref(), self.renderer.as_mut()) else {
            return;
        };
        renderer.set_viewport(gpu, physical[0], physical[1]);
        let Some(surface) = self.surface.as_ref() else {
            return;
        };
        let frame = match surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                if let Some(config) = self.config.as_ref() {
                    surface.configure(&gpu.device, config);
                }
                return;
            }
            Err(error) => {
                crate::gpu::note_error(format!("the window surface failed: {error}"));
                return;
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let background = theme::color::APP_BG;
        renderer.render(
            gpu,
            &view,
            &self.painter,
            wgpu::Color {
                r: f64::from(background.r),
                g: f64::from(background.g),
                b: f64::from(background.b),
                a: 1.0,
            },
        );
        // A frame that is dropped instead of presented leaves a blank window.
        frame.present();
    }

    fn tick(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f64().min(0.1);
        self.last_frame = now;
        self.update_rate(dt);
        if let Some((_, remaining)) = self.banner.as_mut() {
            *remaining -= dt;
            if *remaining <= 0.0 {
                self.banner = None;
            }
        }
        self.advance(dt);
        self.draw();
    }
}

/// The save files in the working directory, for Load to cycle through.
fn save_files() -> Vec<PathBuf> {
    std::fs::read_dir(SAVES_DIR)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .is_some_and(|extension| extension == "json")
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Seed choice is not part of a run: it comes from the clock, never from a
/// seeded stream, so "New seed" is genuinely new.
fn fresh_seed() -> u32 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos() ^ duration.as_secs() as u32)
        .unwrap_or(0);
    nanos.max(1)
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("NeuroArena")
            .with_inner_size(LogicalSize::new(1440.0, 720.0))
            .with_min_inner_size(LogicalSize::new(900.0, 560.0));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                eprintln!("NeuroArena cannot create a window: {error}");
                event_loop.exit();
                return;
            }
        };

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            flags: wgpu::InstanceFlags::default(),
            backend_options: wgpu::BackendOptions::default(),
        });
        let surface = match instance.create_surface(window.clone()) {
            Ok(surface) => surface,
            Err(error) => {
                report_startup_failure(&window, event_loop, &format!("no window surface: {error}"));
                return;
            }
        };
        let gpu = match Gpu::new(instance, Some(&surface)) {
            Ok(gpu) => gpu,
            Err(error) => {
                report_startup_failure(&window, event_loop, &error);
                return;
            }
        };
        let capabilities = surface.get_capabilities(&gpu.adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .unwrap_or(capabilities.formats[0]);
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: capabilities.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&gpu.device, &config);
        if let Some(error) = crate::gpu::take_error() {
            report_startup_failure(
                &window,
                event_loop,
                &format!("the window surface could not be configured: {error}"),
            );
            return;
        }
        self.renderer = Some(Renderer::new(&gpu, format));
        self.surface_format = format;
        self.surface = Some(surface);
        self.config = Some(config);
        self.gpu = Some(gpu);
        self.window = Some(window);
        self.ready = true;
        self.start_generation();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let (Some(surface), Some(config), Some(gpu)) = (
                    self.surface.as_ref(),
                    self.config.as_mut(),
                    self.gpu.as_ref(),
                ) {
                    config.width = size.width.max(1);
                    config.height = size.height.max(1);
                    surface.configure(&gpu.device, config);
                }
            }
            WindowEvent::CursorMoved { position, .. } => self.on_move(position),
            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Left {
                    match state {
                        ElementState::Pressed => self.on_press(),
                        ElementState::Released => self.dragging_slider = false,
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.on_key(&event.logical_key, event.state)
            }
            WindowEvent::RedrawRequested => self.tick(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }
}

/// A GPU that cannot be initialised is shown, not swallowed: the window says
/// what went wrong before the app gives up on it.
fn report_startup_failure(window: &Window, event_loop: &ActiveEventLoop, error: &str) {
    window.set_title(&format!("NeuroArena — {error}"));
    eprintln!("NeuroArena cannot draw: {error}");
    event_loop.exit();
}
