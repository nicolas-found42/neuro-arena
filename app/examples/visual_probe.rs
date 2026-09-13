//! Deterministic native GPU capture and synchronized renderer timing.
//! cargo run --release -p neuroarena-app --example visual_probe -- /tmp/frame.png 1440 720 180
//!
//! Optional trailing words: `rays` overlays the nine Sensor Rays, `still` is
//! the reduced-motion frame, `dense` substitutes a stress topology.
use neuroarena_app::{
    gpu::{Gpu, Offscreen},
    observatory,
    painter::{Painter, Transform},
    panels,
    renderer::Renderer,
    scene, ui,
};
use sim::{Competence, Run, RunOptions};
use std::time::Instant;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let path = args
        .get(1)
        .map(String::as_str)
        .unwrap_or("/tmp/neuro-frame.png");
    let width: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1440);
    let height: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(720);
    let find_impact = args.get(4).is_some_and(|s| s == "impact");
    let steps: usize = if find_impact {
        3600
    } else {
        args.get(4).and_then(|s| s.parse().ok()).unwrap_or(180)
    };
    let dpr: f32 = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(1.0);
    let completed: usize = args.get(6).and_then(|s| s.parse().ok()).unwrap_or(0);
    let member: usize = args.get(7).and_then(|s| s.parse().ok()).unwrap_or(0);
    let mut run = Run::with_options(2026, RunOptions::new(100, 1));
    for _ in 0..completed {
        let g = run.begin_generation();
        run.complete_generation(g.evaluate_all(1));
    }
    let generation = run.begin_generation();
    // The Record's cloud draws the Generation that is on the field, so the
    // probe has to have evaluated one: this is the same work the app's worker
    // pool does, done here so a capture can show a full cohort.
    let outcomes = generation.evaluate_all(sim::default_workers());
    let mut roster = neuroarena_app::cohort::Cohort::default();
    let roster_size = outcomes.len();
    roster.begin(run.generation(), roster_size, member);
    // The watched member is stepped below, so its slot is filled then; the
    // rest are banked here, which is what the app's pool reports.
    for outcome in outcomes.iter().filter(|o| o.member != member) {
        roster.record(outcome.member, outcome);
    }
    let mut world = generation.world(member);
    let mut inspected_genome = run.population().genomes[member].clone();
    if args.get(8).is_some_and(|s| s == "dense") {
        let mut rng = sim::Rng::from_seed(44);
        let mut tracker = sim::InnovationTracker::from_genome(&inspected_genome);
        for _ in 0..48 {
            inspected_genome.mutate_add_node(&mut rng, &mut tracker);
        }
        world = sim::World::new(
            sim::Rng::from_seed(2026),
            Some(sim::Network::from_genome(&inspected_genome)),
        );
    }
    // `still` is the reduced-motion frame: no trails, no echoes, no tremor.
    let motion = !args.iter().any(|arg| arg == "still");
    // The Ray overlay is opt-in in the window, so it is opt-in here too: a
    // capture that always wore nine bearings across the field would not be the
    // frame the app draws by default.
    let rays = args.iter().any(|arg| arg == "rays");
    let mut trail = observatory::Trail::default();
    let mut effects = neuroarena_app::effects::Effects::default();
    let mut first_impact = None;
    for step in 0..steps {
        if world.done {
            break;
        }
        let frame_start = world.time;
        world.step_fixed();
        let key = (run.generation(), member);
        trail.observe(key, &world, motion);
        effects.observe(key, &world, motion);
        if world.impacts().next().is_some() && first_impact.is_none() {
            first_impact = Some(step);
        }
        if find_impact && first_impact.is_some_and(|first| step >= first + 6) {
            break;
        }
        // One frame of stepping, committed the way the shell commits it: the
        // capture has to see the same events the window would.
        trail.settle(world.time - frame_start);
        effects.settle(motion);
    }
    // The last frame's worth of events is what the frame draws with, so the
    // tremor the Arena is drawn under is read after the settle, as in the shell.
    let tremor = effects.shake();
    // The watched Episode banks the moment it ends, exactly as the shell does.
    roster.record(member, &generation.outcome(member, &world));
    let gpu = Gpu::new(
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle()),
        None,
    )
    .unwrap();
    let target = Offscreen::new(&gpu, width, height, wgpu::TextureFormat::Rgba8UnormSrgb);
    let mut renderer = Renderer::new(&gpu, target.format);
    let layout = ui::Layout::new(width as f32 / dpr, height as f32 / dpr);
    let arena = observatory::arena_view(layout.arena, dpr).rect();
    renderer.set_frame(
        &gpu,
        neuroarena_app::renderer::Frame {
            viewport: [width as f32, height as f32],
            arena: [arena.0, arena.1, arena.2, arena.3],
        },
    );
    let mut painter = Painter::new();
    painter.set_transform(Transform::new(dpr, [0.0, 0.0]));
    // As in the app: the frame draws with the offset the effects pass
    // observed, while the placement rect stays still.
    let view = neuroarena_app::scene::ArenaView {
        tremor,
        ..observatory::arena_view(layout.arena, dpr)
    };
    scene::draw_arena(&mut painter, &world, view, rays, motion);
    let before_ribbon = painter.luminous.len();
    trail.draw(&mut painter, view);
    let ribbon = painter.luminous.len() - before_ribbon;
    // How far the trace actually reaches, in window pixels: the one number that
    // says whether a capture is showing a ribbon or the ship standing still.
    let span = {
        let vertices = &painter.luminous[before_ribbon..];
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for vertex in vertices {
            for axis in 0..2 {
                lo[axis] = lo[axis].min(vertex.pos[axis]);
                hi[axis] = hi[axis].max(vertex.pos[axis]);
            }
        }
        if vertices.is_empty() {
            0.0
        } else {
            (hi[0] - lo[0]).hypot(hi[1] - lo[1])
        }
    };
    effects.draw(&mut painter, view);
    painter.set_transform(Transform::new(dpr, [0.0, 0.0]));
    let info = panels::HudInfo {
        seed: 2026,
        generation: run.generation(),
        member,
        population: 100,
        episode_time: world.time,
        wave: world.wave,
        fitness: world.agent.fitness,
        competence: Competence {
            alive_time: world.agent.stats.alive_time,
            wave: world.wave,
            asteroids: world.agent.stats.asteroid_points,
        },
        gate: run.gate(),
        species: run.population().species.len(),
        speed: 1.0,
        measured_rate: 0.0,
        watching: false,
    };
    panels::draw_hud(&mut painter, layout.hud, &info);
    panels::draw_record(&mut painter, layout.record, run.history(), &roster);
    panels::draw_network(
        &mut painter,
        layout.network,
        &inspected_genome,
        world.agent.network().unwrap(),
    );
    let controls = ui::Controls {
        paused: true,
        rays,
        motion,
        speed: 1.0,
        seed_text: "2026".into(),
        ..Default::default()
    };
    observatory::draw_chrome(
        &mut painter,
        layout.arena,
        Some(&world),
        2026,
        &controls,
        true,
    );
    panels::draw_controls(&mut painter, &layout, &controls, None);
    panels::draw_status(
        &mut painter,
        layout.status,
        if args.get(8).is_some_and(|s| s == "dense") {
            "seed 2026 / topology stress fixture"
        } else {
            "seed 2026 / fixed step capture"
        },
        false,
    );
    let mut times = Vec::new();
    for i in 0..130 {
        let start = Instant::now();
        renderer.render(&gpu, target.view(), &painter);
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        if i >= 10 {
            times.push(start.elapsed().as_secs_f64() * 1000.0);
        }
    }
    times.sort_by(f64::total_cmp);
    println!(
        "adapter={} size={}x{} actual_step={} triangles={} glyphs={} ribbon_tris={} ribbon_span={:.0}px render_ms_p50={:.3} p95={:.3}",
        gpu.info.name,
        width,
        height,
        (world.time / sim::DT).round() as usize,
        painter.triangles.len() / 3,
        renderer.last_glyphs,
        ribbon,
        span,
        times[60],
        times[114]
    );
    let file = std::fs::File::create(path).unwrap();
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&target.read_rgba(&gpu))
        .unwrap();
}
