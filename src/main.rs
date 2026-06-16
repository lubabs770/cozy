//! cozy — animated rain over your Wayland wallpaper.
//!
//! cozy is a `wlr-layer-shell` client that sits on the **Background** layer and
//! (eventually) renders the wallpaper itself plus two rain effects on top of it:
//! additive falling streaks and refracting glass droplets, all in one fragment
//! shader.
//!
//! ## Layout
//!
//! * [`app`] — application state and all Wayland event handlers.
//! * [`surface`] — one Background layer surface per output, plus its GL drawing.
//! * [`render`] — EGL/GLES context management (and, later, the shader pipeline).
//!
//! ## Milestone status
//!
//! M2: each surface brings up an OpenGL ES 3.0 context and clears to a solid
//! color, proving the EGL pipeline works. M3 swaps the clear for the wallpaper
//! texture; M4 adds the rain.

mod app;
mod render;
mod surface;

use std::rc::Rc;

use anyhow::{Context, Result};
use smithay_client_toolkit::{
    compositor::CompositorState, output::OutputState, registry::RegistryState,
    shell::wlr_layer::LayerShell,
};
use wayland_client::{globals::registry_queue_init, Connection};

use app::Cozy;
use render::egl::Egl;

/// The wallpaper, embedded for now. M6 will let config point at an arbitrary
/// file; until then cozy ships with this test image so it renders out of the box.
const WALLPAPER: &[u8] = include_bytes!("../assets/test-wallpaper.png");

fn main() -> Result<()> {
    let conn = Connection::connect_to_env().context("connect to Wayland display")?;
    let (globals, mut event_queue) =
        registry_queue_init(&conn).context("initialize Wayland registry")?;
    let qh = event_queue.handle();

    let compositor = CompositorState::bind(&globals, &qh).context("wl_compositor not available")?;
    let layer_shell =
        LayerShell::bind(&globals, &qh).context("zwlr_layer_shell_v1 not available")?;
    let egl = Rc::new(Egl::new(&conn).context("initialize EGL")?);

    let mut state = Cozy::new(
        RegistryState::new(&globals),
        OutputState::new(&globals, &qh),
        compositor,
        layer_shell,
        egl,
        WALLPAPER,
        qh.clone(),
    );

    // Surfaces are created from OutputHandler::new_output as outputs are
    // announced; pump the queue until we have at least one.
    loop {
        event_queue.blocking_dispatch(&mut state)?;
        if state.has_surfaces() {
            break;
        }
    }

    loop {
        event_queue.blocking_dispatch(&mut state)?;
    }
}
