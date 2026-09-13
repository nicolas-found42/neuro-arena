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
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use sim::{
    config, default_workers, EpisodeOutcome, Generation, GenomeFile, Run, RunOptions, World,
};

use crate::gpu::Gpu;
use crate::observatory::{self, Trail};
use crate::painter::{Painter, Transform};
use crate::renderer::{Frame, Renderer};
use crate::scene;
use crate::{panels, theme, ui};

/// How long a frame may spend stepping the watched Episode, in seconds. This is
/// what keeps the frame rate steady while the run goes as fast as it can.
const SIM_BUDGET: f64 = 0.008;
/// The window the measured rate is averaged over.
const RATE_WINDOW: f64 = 1.0;
/// How long a Generation-complete banner stays up.
const BANNER_SECONDS: f64 = 1.2;
/// The banner fades in over its first moments and out over its last; reduced
/// motion shows it at full strength immediately.
const BANNER_FADE_IN: f64 = 0.15;
const BANNER_FADE_OUT: f64 = 0.30;
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
    trail: Trail,
    effects: crate::effects::Effects,
    /// The Arena's displacement, read from the effects pass after it observes
    /// and carried on the view the next frame draws with. A held frame keeps
    /// the offset it was given, which is what makes a paused frame
    /// pixel-identical.
    arena_tremor: [f32; 2],
    trails: bool,

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
    focus: Option<ui::Hit>,
    pressed: Option<ui::Hit>,
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
    /// Set by every event that changes what the window would show, cleared once
    /// a frame has been drawn. Paused and otherwise idle this is the only thing
    /// besides a banner still fading that earns a frame: the last one stands.
    redraw_pending: bool,
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
            trail: Trail::default(),
            effects: crate::effects::Effects::default(),
            arena_tremor: [0.0, 0.0],
            trails: true,
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
            focus: None,
            pressed: None,
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
            redraw_pending: false,
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
        self.trail.clear();
        self.effects.clear();
        // The field the next frame draws is the new run's, not the dead one's.
        self.arena_tremor = [0.0, 0.0];
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
        if self.generation.is_none() {
            self.start_generation();
        }
        if self.controls.paused {
            return;
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
                if self.trails && speed <= 16.0 {
                    self.effects
                        .observe((self.run.generation(), self.watched_member), world, true);
                }
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

    // ---- the loop --------------------------------------------------------

    /// Does the loop owe the window another frame? While the run is moving,
    /// always: the Arena is animating. While it is paused only two things earn
    /// a frame — a banner with moments left to fade, and an event that landed
    /// since the last frame was drawn. Otherwise the frame already on the
    /// surface is the right one and stands.
    fn wants_redraw(&self) -> bool {
        !self.controls.paused || self.banner.is_some() || self.redraw_pending
    }

    /// Ask for one frame now, rather than wait for the next turn of the loop.
    /// Events call this so that a paused window answers a click, a key or a
    /// resize in the same turn; `about_to_wait` asks again for as long as the
    /// frame the window shows is still out of date.
    fn request_frame(&mut self) {
        self.redraw_pending = true;
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    /// Take a new physical size for the surface and reconfigure it with it, so
    /// the next frame covers the whole window.
    fn set_surface_size(&mut self, width: u32, height: u32) {
        let Some(config) = self.config.as_mut() else {
            return;
        };
        config.width = width.max(1);
        config.height = height.max(1);
        if let (Some(surface), Some(gpu)) = (self.surface.as_ref(), self.gpu.as_ref()) {
            surface.configure(&gpu.device, config);
        }
    }

    /// The window's scale factor changed — a move to another display, usually.
    /// The physical size the window reports now is the one the surface takes,
    /// and a frame is asked for: the layout divides by the live factor, so that
    /// one frame picks up both the new size and the new dpr.
    fn on_scale_factor(&mut self, size: Option<PhysicalSize<u32>>) {
        if let Some(size) = size {
            self.set_surface_size(size.width, size.height);
        }
        self.request_frame();
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
        // Controls point at themselves: the cursor says "clickable" only
        // where a control actually sits.
        if let Some(window) = self.window.as_ref() {
            let icon = if self.hot.is_some() {
                winit::window::CursorIcon::Pointer
            } else {
                winit::window::CursorIcon::Default
            };
            window.set_cursor(icon);
        }
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
        self.focus = Some(hit);
        self.pressed = Some(hit);
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
            Key::Named(NamedKey::Tab) => {
                let index = self
                    .focus
                    .and_then(|hit| ui::FOCUS_ORDER.iter().position(|h| *h == hit))
                    .map_or(0, |i| (i + 1) % ui::FOCUS_ORDER.len());
                self.focus = Some(ui::FOCUS_ORDER[index]);
                self.controls.seed_editing = self.focus == Some(ui::Hit::SeedField);
            }
            Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowRight)
                if self.focus == Some(ui::Hit::SpeedSlider) =>
            {
                let direction = if *key == Key::Named(NamedKey::ArrowRight) {
                    0.025
                } else {
                    -0.025
                };
                self.controls.speed = ui::Controls::speed_from_slider(
                    ui::Controls::slider_from_speed(self.controls.speed) + direction,
                );
            }
            Key::Named(NamedKey::Enter)
                if self.focus.is_some() && self.focus != Some(ui::Hit::SeedField) =>
            {
                if let (Some(hit), Some(layout)) = (self.focus, self.layout()) {
                    let point = layout.control_rect(hit).center();
                    let cursor = self.cursor;
                    self.cursor = Some(PhysicalPosition::new(
                        (point[0] * self.dpr()) as f64,
                        (point[1] * self.dpr()) as f64,
                    ));
                    self.on_press();
                    self.cursor = cursor;
                    self.pressed = None;
                    self.dragging_slider = false;
                }
            }
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
                // A digit belongs to the seed field, and only when the field is
                // where the eye is: being edited, focused by Tab or a click, or
                // under the cursor. Anywhere else a digit is not a command and
                // is dropped rather than opening the field behind the user.
                let field = self.controls.seed_editing
                    || self.focus == Some(ui::Hit::SeedField)
                    || self.hot == Some(ui::Hit::SeedField);
                let mut typed = false;
                for character in text.chars() {
                    if character.is_ascii_digit() {
                        if !field {
                            continue;
                        }
                        self.controls.seed_editing = true;
                        self.controls.push_seed_char(character);
                        typed = true;
                    }
                }
                if typed {
                    return;
                }
                // While the field is being edited the field has the keyboard:
                // every key belongs to it, so no letter is also a command.
                if self.controls.seed_editing {
                    return;
                }
                match text.to_lowercase().as_str() {
                    "m" => {
                        self.trails = !self.trails;
                        self.trail.clear();
                        self.effects.clear();
                        self.arena_tremor = [0.0, 0.0];
                    }
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

    /// Fill the Painter for this frame and return the geometry the renderer
    /// needs: the window's physical extent and where the Arena sits in it.
    fn build_frame(&mut self) -> Frame {
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
        // The window's ground is painted by the backdrop pass, which knows
        // where the Arena is and lays the deep field inside it.

        // The Arena keeps its 960×600 shape and scales into whatever space the
        // sidebar leaves; the panels stay at 1:1 and stay legible (ADR 0006).
        let arena_view = observatory::arena_view(layout.arena, dpr);
        if let Some(world) = self.watched.as_ref() {
            // The tremor the frame draws with is the one the frame before it
            // observed; the offset is on the view, not on global state.
            let view = crate::scene::ArenaView {
                tremor: self.arena_tremor,
                ..arena_view
            };
            self.trail.observe(
                (self.run.generation(), self.watched_member),
                world,
                self.trails,
            );
            scene::draw_arena(painter, world, view, self.controls.rays, self.trails);
            self.trail.draw(painter, view);
            self.effects.observe(
                (self.run.generation(), self.watched_member),
                world,
                self.trails && self.controls.speed <= 16.0,
            );
            self.arena_tremor = self.effects.shake();
            self.effects.draw(painter, view);
        }
        painter.set_transform(Transform::new(dpr, [0.0, 0.0]));

        let world = self.watched.as_ref();
        observatory::draw_chrome(
            painter,
            layout.arena,
            world,
            self.seed,
            &self.controls,
            self.trails,
        );
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
        if let Some(hit) = self.focus {
            draw_focus_ring(painter, ui::focus_ring(layout.control_rect(hit)));
        }
        if let Some(hit) = self.pressed {
            let r = layout.control_rect(hit);
            painter.rect(r.x, r.y, r.w, r.h, theme::color::ACCENT.alpha(0.12));
        }
        let (status, is_error) = match crate::gpu::take_error() {
            Some(error) => (error, true),
            None => (self.message.clone(), self.message_is_error),
        };
        panels::draw_status(painter, layout.status, &status, is_error);
        if let Some((text, remaining)) = self.banner.as_ref() {
            let shown = BANNER_SECONDS - remaining;
            let fade = (shown / BANNER_FADE_IN)
                .min(remaining / BANNER_FADE_OUT)
                .clamp(0.0, 1.0);
            let alpha = if self.trails { fade as f32 } else { 1.0 };
            panels::draw_banner(
                painter,
                layout.arena,
                &[(text.clone(), theme::color::TEXT.alpha(alpha))],
            );
        }
        let (x, y, width, height) = arena_view.rect();
        Frame {
            viewport: physical,
            arena: [x, y, width, height],
        }
    }

    fn draw(&mut self) {
        // The frame the loop owed is being drawn now, whether or not the
        // surface is ready to take it.
        self.redraw_pending = false;
        if !self.ready {
            return;
        }
        let frame = self.build_frame();

        let (Some(gpu), Some(renderer)) = (self.gpu.as_ref(), self.renderer.as_mut()) else {
            return;
        };
        renderer.set_frame(gpu, frame);
        let Some(surface) = self.surface.as_ref() else {
            return;
        };
        let frame = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated => {
                if let Some(config) = self.config.as_ref() {
                    surface.configure(&gpu.device, config);
                }
                return;
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                if let (Some(window), Some(config)) = (self.window.as_ref(), self.config.as_ref()) {
                    match gpu.instance.create_surface(window.clone()) {
                        Ok(surface) => {
                            surface.configure(&gpu.device, config);
                            self.surface = Some(surface);
                        }
                        Err(error) => {
                            crate::gpu::note_error(format!(
                                "could not recreate the window surface: {error}"
                            ));
                        }
                    }
                }
                return;
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return,
            wgpu::CurrentSurfaceTexture::Validation => {
                crate::gpu::note_error("the window surface failed validation");
                return;
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        renderer.render(gpu, &view, &self.painter);
        // A frame that is dropped instead of presented leaves a blank window.
        gpu.queue.present(frame);
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

/// Stroke the focus ring around a control: four segments along the ring rect,
/// submitted after the panels so it reads over whatever the control's own face
/// is doing — hover tint, toggled fill and all.
fn draw_focus_ring(painter: &mut Painter, ring: ui::Rect) {
    let ink = theme::color::ACCENT.alpha(0.8);
    let weight = ui::FOCUS_RING_WEIGHT;
    let corners = [
        [ring.x, ring.y],
        [ring.right(), ring.y],
        [ring.right(), ring.bottom()],
        [ring.x, ring.bottom()],
    ];
    for (from, to) in [(0, 1), (1, 2), (2, 3), (3, 0)] {
        painter.stroke(corners[from], corners[to], weight, ink);
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
            .with_inner_size(LogicalSize::new(1440.0, 900.0))
            .with_min_inner_size(LogicalSize::new(900.0, 560.0));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                eprintln!("NeuroArena cannot create a window: {error}");
                event_loop.exit();
                return;
            }
        };

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(
            Box::new(event_loop.owned_display_handle()),
        ));
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
            color_space: wgpu::SurfaceColorSpace::Auto,
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
                self.set_surface_size(size.width, size.height);
                self.request_frame();
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                // The OS resizes the window along with the factor, so whatever
                // it reports as the physical size now is authoritative.
                let size = self.window.as_ref().map(|window| window.inner_size());
                self.on_scale_factor(size);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.on_move(position);
                self.request_frame();
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Left {
                    match state {
                        ElementState::Pressed => self.on_press(),
                        ElementState::Released => {
                            self.dragging_slider = false;
                            self.pressed = None;
                        }
                    }
                    self.request_frame();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                // A release changes nothing the frame draws, and the window
                // does not need waking for it.
                if event.state == ElementState::Pressed {
                    self.request_frame();
                }
                self.on_key(&event.logical_key, event.state)
            }
            WindowEvent::RedrawRequested => self.tick(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if self.wants_redraw() {
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::layout::SIDEBAR_WIDTH;

    /// An app with nothing but its own state: no window and no GPU, which is
    /// all the loop's questions and the input path need answered.
    fn app() -> App {
        App::new()
    }

    fn character(text: &str) -> Key {
        Key::Character(text.into())
    }

    fn press(app: &mut App, key: Key) {
        app.on_key(&key, ElementState::Pressed);
    }

    /// A surface configuration as `resumed` builds one — enough for `layout`
    /// to read a window size from, without a window behind it.
    fn configuration(width: u32, height: u32) -> wgpu::SurfaceConfiguration {
        wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            color_space: wgpu::SurfaceColorSpace::Auto,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        }
    }

    fn overlaps(a: ui::Rect, b: ui::Rect) -> bool {
        a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
    }

    #[test]
    fn the_loop_owes_a_frame_only_what_can_change() {
        let mut app = app();

        // Running: the Arena is animating, so every turn asks for a frame.
        assert!(app.wants_redraw(), "a running run always wants a frame");

        // Paused with nothing new: the frame already on the surface stands.
        app.controls.paused = true;
        assert!(!app.wants_redraw(), "a paused, idle loop draws nothing");

        // A banner still fading has to be seen out.
        app.banner = Some((
            "Generation 4 · best fitness 12.0".to_string(),
            BANNER_SECONDS,
        ));
        assert!(app.wants_redraw(), "a fading banner earns frames");
        app.banner = None;

        // An event that lands while paused earns exactly one frame: the event
        // asks for it, and drawing that frame settles it.
        app.request_frame();
        assert!(app.wants_redraw(), "a pending event earns a frame");
        app.draw();
        assert!(!app.wants_redraw(), "the pending frame has been drawn");

        // Running again is continuous.
        app.controls.paused = false;
        assert!(app.wants_redraw(), "resuming asks for frames again");
    }

    #[test]
    fn space_pauses_with_one_frame_to_show_for_it() {
        let mut app = app();
        press(&mut app, Key::Named(NamedKey::Space));
        assert!(app.controls.paused);
        // The key that paused is what asks for the frame showing the PAUSED
        // lamp; once it is drawn the loop goes quiet and the lamp holds.
        app.request_frame();
        assert!(app.wants_redraw());
        app.draw();
        assert!(!app.wants_redraw());

        press(&mut app, Key::Named(NamedKey::Space));
        assert!(!app.controls.paused);
        assert!(app.wants_redraw(), "resuming is continuous again");
    }

    #[test]
    fn the_seed_field_takes_the_keyboard_only_where_it_is() {
        let mut app = app();
        app.controls.seed_text = "2026".to_string();
        app.controls.seed_editing = false;
        app.focus = None;
        app.hot = None;

        // Nowhere near the field: a digit is not a command, and does not open
        // the field behind the user's back.
        press(&mut app, character("7"));
        assert_eq!(app.controls.seed_text, "2026");
        assert!(!app.controls.seed_editing);

        // Hovered: a digit enters the field and is typed into it.
        app.hot = Some(ui::Hit::SeedField);
        press(&mut app, character("7"));
        assert!(app.controls.seed_editing);
        assert_eq!(app.controls.seed_text, "20267");

        // Editing: digits land, and no letter is also a command.
        press(&mut app, character("8"));
        assert_eq!(app.controls.seed_text, "202678");
        let rays = app.controls.rays;
        press(&mut app, character("r"));
        assert_eq!(
            app.controls.rays, rays,
            "'r' is text while the field is edited"
        );
        assert_eq!(app.controls.seed_text, "202678");

        // The pointer leaving does not end the edit: the caret keeps the keys.
        app.hot = None;
        press(&mut app, character("9"));
        assert_eq!(app.controls.seed_text, "2026789");

        // Escape leaves the field with what was typed; commands return.
        press(&mut app, Key::Named(NamedKey::Escape));
        assert!(!app.controls.seed_editing);
        assert_eq!(app.controls.seed_text, "2026789", "the digits are kept");
        press(&mut app, character("r"));
        assert_ne!(
            app.controls.rays, rays,
            "'r' toggles Rays once the field is left"
        );
        press(&mut app, character("m"));
        assert!(!app.trails, "'m' toggles Trails too");

        // Tab reaches the field and puts the caret in it.
        app.focus = None;
        let mut tabs = 0;
        while app.focus != Some(ui::Hit::SeedField) && tabs <= ui::FOCUS_ORDER.len() {
            press(&mut app, Key::Named(NamedKey::Tab));
            tabs += 1;
        }
        assert_eq!(app.focus, Some(ui::Hit::SeedField));
        assert!(
            app.controls.seed_editing,
            "Tab into the field starts editing"
        );

        // Focused but not editing: a digit re-enters the field.
        press(&mut app, Key::Named(NamedKey::Escape));
        assert!(!app.controls.seed_editing);
        press(&mut app, character("4"));
        assert!(app.controls.seed_editing);
        assert_eq!(app.controls.seed_text, "20267894");
    }

    #[test]
    fn every_letter_command_survives_the_match_it_lives_in() {
        // The five letter hotkeys are one match; a refactoring edit has more
        // than once replaced the whole arm alongside its neighbour. Each is
        // pinned by its own observable state.
        let mut app = app();
        app.controls.seed_editing = false;
        app.focus = None;

        let rays = app.controls.rays;
        press(&mut app, character("r"));
        assert_ne!(app.controls.rays, rays, "'r' toggles Rays");

        let trails = app.trails;
        press(&mut app, character("m"));
        assert_ne!(app.trails, trails, "'m' toggles Trails");

        // The pin is "the arm still exists and speaks", not "the IO
        // succeeded": the observable is the status line, which changes
        // whether the directory is empty or not. Hermetic by construction —
        // a fresh run has finished no Generation, so 's' has nothing to write
        // and touches no disk, and 'l' only reads.
        let before = app.message.clone();
        press(&mut app, character("s"));
        assert_ne!(app.message, before, "'s' reports through the status line");

        // Load reports whether it found a Genome to watch or an empty
        // directory; Evolve names the Genome it would need, or the one it is
        // resuming from. Either way the command spoke.
        let before = app.message.clone();
        press(&mut app, character("l"));
        assert_ne!(app.message, before, "'l' reports through the status line");

        let before = app.message.clone();
        press(&mut app, character("e"));
        assert_ne!(app.message, before, "'e' reports through the status line");
    }

    #[test]
    fn the_focus_ring_clears_the_control_it_belongs_to() {
        // The stroke's band straddles the ring line by half its weight.
        let band = ui::FOCUS_RING_WEIGHT * 0.5;
        for (width, height) in [(900.0, 560.0), (1280.0, 800.0), (3840.0, 2160.0)] {
            let layout = ui::Layout::new(width, height);
            for hit in ui::FOCUS_ORDER {
                let face = layout.control_rect(hit);
                let ring = ui::focus_ring(face);
                assert!(
                    ring.x < face.x && ring.y < face.y,
                    "{hit:?} ring is not outside its face in {width}×{height}: {face:?} {ring:?}"
                );
                assert!(
                    ring.right() > face.right() && ring.bottom() > face.bottom(),
                    "{hit:?} ring is not outside its face in {width}×{height}: {face:?} {ring:?}"
                );
                // The whole band clears the face, so a control filled edge to
                // edge — a toggled `On` key — can never cover the ring.
                assert!(
                    ring.x + band <= face.x
                        && ring.y + band <= face.y
                        && ring.right() - band >= face.right()
                        && ring.bottom() - band >= face.bottom(),
                    "{hit:?} ring band touches its face in {width}×{height}: {face:?} {ring:?}"
                );
                // And it stays inside the panel the control lives in.
                assert!(
                    ring.x >= layout.controls.x
                        && ring.y >= layout.controls.y
                        && ring.right() <= layout.controls.right()
                        && ring.bottom() <= layout.controls.bottom(),
                    "{hit:?} ring escapes the controls panel in {width}×{height}: {ring:?}"
                );
                // At the natural strip size the ring is narrower than the gap
                // between cells, so it never lands on a neighbour either.
                if height >= 900.0 {
                    for other in ui::FOCUS_ORDER {
                        let neighbour = layout.control_rect(other);
                        assert!(
                            other == hit || !overlaps(ring, neighbour),
                            "{hit:?} ring lands on {other:?} in {width}×{height}: {ring:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_scale_factor_change_re_lays_out_and_asks_for_a_frame() {
        let mut app = app();
        app.config = Some(configuration(800, 600));
        app.controls.paused = true;
        assert_eq!(
            app.layout()
                .expect("a configured surface has a layout")
                .sidebar
                .x,
            800.0 - SIDEBAR_WIDTH
        );
        assert!(!app.wants_redraw());

        // The window moved to a display with twice the dpr: the OS resizes it
        // along with the factor, and the layout — which divides by the live
        // factor — has to be recomputed from the size that comes with the
        // event.
        app.on_scale_factor(Some(PhysicalSize::new(1600, 1200)));
        assert_eq!(app.config.as_ref().expect("configured").width, 1600);
        assert_eq!(
            app.layout().expect("still configured").sidebar.x,
            1600.0 - SIDEBAR_WIDTH,
            "the panels moved to the new window width"
        );
        assert!(
            app.wants_redraw(),
            "the change asks for a frame, paused or not"
        );
        app.draw();
        assert!(!app.wants_redraw());
    }
}
