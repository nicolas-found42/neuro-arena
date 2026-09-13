//! Lyon supplies joined strokes and adaptive curve subdivision; wgpu stays the renderer.
use crate::painter::{Painter, Rgba};
use lyon_tessellation::{
    math::point, path::Path, BuffersBuilder, StrokeOptions, StrokeTessellator, StrokeVertex,
    VertexBuffers,
};

pub struct Paths {
    tessellator: StrokeTessellator,
    mesh: VertexBuffers<[f32; 2], u32>,
}
impl Default for Paths {
    fn default() -> Self {
        Self {
            tessellator: StrokeTessellator::new(),
            mesh: VertexBuffers::new(),
        }
    }
}
impl Paths {
    pub fn draw(&mut self, p: &mut Painter, path: &Path, width: f32, color: Rgba) {
        self.mesh.vertices.clear();
        self.mesh.indices.clear();
        let options = StrokeOptions::default()
            .with_line_width(width)
            .with_tolerance(0.25 / p.transform().scale.max(0.1));
        self.tessellator
            .tessellate_path(
                path,
                &options,
                &mut BuffersBuilder::new(&mut self.mesh, |v: StrokeVertex| v.position().to_array()),
            )
            .expect("finite instrument paths");
        for t in self.mesh.indices.as_chunks::<3>().0 {
            p.triangle(
                self.mesh.vertices[t[0] as usize],
                self.mesh.vertices[t[1] as usize],
                self.mesh.vertices[t[2] as usize],
                color,
            );
        }
    }
}

pub fn connection(a: [f32; 2], b: [f32; 2], dashed: bool) -> Path {
    let mut path = Path::builder();
    let mid = (a[0] + b[0]) * 0.5;
    let curve = lyon_tessellation::geom::CubicBezierSegment {
        from: point(a[0], a[1]),
        ctrl1: point(mid, a[1]),
        ctrl2: point(mid, b[1]),
        to: point(b[0], b[1]),
    };
    let segments = if dashed { 12 } else { 1 };
    for i in 0..segments {
        let part = curve.split_range(
            i as f32 / segments as f32
                ..(i as f32 + if dashed { 0.6 } else { 1.0 }) / segments as f32,
        );
        path.begin(part.from);
        path.cubic_bezier_to(part.ctrl1, part.ctrl2, part.to);
        path.end(false);
    }
    path.build()
}

pub fn arc(center: [f32; 2], radius: f32, start: f32, sweep: f32) -> Path {
    let mut path = Path::builder();
    let at = |a: f32| point(center[0] + radius * a.cos(), center[1] + radius * a.sin());
    path.begin(at(start));
    // Cubic circular segments of at most 90 degrees; Lyon flattens for the actual DPI.
    let segments = (sweep.abs() / std::f32::consts::FRAC_PI_2).ceil().max(1.0) as usize;
    let delta = sweep / segments as f32;
    for i in 0..segments {
        let a = start + delta * i as f32;
        let b = a + delta;
        let k = 4.0 / 3.0 * (delta * 0.25).tan() * radius;
        let pa = at(a);
        let pb = at(b);
        path.cubic_bezier_to(
            point(pa.x - k * a.sin(), pa.y + k * a.cos()),
            point(pb.x + k * b.sin(), pb.y - k * b.cos()),
            pb,
        );
    }
    path.end(false);
    path.build()
}
