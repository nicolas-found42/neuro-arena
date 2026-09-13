//! NeuroArena: the native macOS window, the renderer and the panels.
//!
//! The simulation lives in the `sim` crate, which has no windowing or GPU
//! dependency — this crate owns the window, the GPU and the UI, and nothing
//! else (ADR 0004). The library target exists so the offscreen golden-frame
//! test can drive the real renderer without a window.

pub mod appstate;
pub mod atlas;
pub mod effects;
pub mod gpu;
pub mod instruments;
pub mod observatory;
pub mod painter;
pub mod panels;
pub mod renderer;
pub mod scene;
pub mod text;
pub mod theme;
pub mod ui;
pub mod vector;

/// Open the window and run until the user closes it.
pub fn run() {
    appstate::run();
}
