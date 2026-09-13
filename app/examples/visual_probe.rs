//! Deterministic native GPU capture and synchronized renderer timing.
//! cargo run --release -p neuroarena-app --example visual_probe -- /tmp/frame.png 1440 720 180
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
    let mut trail = observatory::Trail::default();
    let mut effects = neuroarena_app::effects::Effects::default();
    let mut first_impact = None;
    for step in 0..steps {
        if world.done {
            break;
        }
        world.step_fixed();
        trail.observe((run.generation(), member), &world, true);
        effects.observe((run.generation(), member), &world, true);
        if world.impacts().next().is_some() && first_impact.is_none() {
            first_impact = Some(step);
        }
        if find_impact && first_impact.is_some_and(|first| step >= first + 6) {
            break;
        }
    }
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
    scene::draw_arena(
        &mut painter,
        &world,
        observatory::arena_view(layout.arena, dpr),
        true,
        true,
    );
    trail.draw(&mut painter, observatory::arena_view(layout.arena, dpr));
    effects.draw(&mut painter, observatory::arena_view(layout.arena, dpr));
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
    panels::draw_chart(&mut painter, layout.chart, run.history());
    panels::draw_network(
        &mut painter,
        layout.network,
        &inspected_genome,
        world.agent.network().unwrap(),
    );
    let controls = ui::Controls {
        paused: true,
        rays: true,
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
        "adapter={} size={}x{} actual_step={} triangles={} glyphs={} render_ms_p50={:.3} p95={:.3}",
        gpu.info.name,
        width,
        height,
        (world.time / sim::DT).round() as usize,
        painter.triangles.len() / 3,
        renderer.last_glyphs,
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
