//! cozy — animated rain over your Wayland wallpaper.
//!
//! cozy is a `wlr-layer-shell` client that sits on the **Background** layer and
//! (eventually) renders the wallpaper itself plus two rain effects on top of it.
//!
//! ## Milestone 1
//!
//! This file currently implements the plumbing milestone only: bring up a
//! fullscreen Background layer surface, paint it solid black via a shared-memory
//! buffer, and give it an **empty input region** so all clicks fall through to
//! the desktop. No EGL/OpenGL yet — that arrives in M2, deliberately kept as a
//! separate problem from the layer-shell plumbing.

use anyhow::{Context, Result};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_output, wl_region, wl_shm, wl_surface},
    Connection, Dispatch, QueueHandle,
};

fn main() -> Result<()> {
    let conn = Connection::connect_to_env().context("connect to Wayland display")?;
    let (globals, mut event_queue) =
        registry_queue_init(&conn).context("initialize Wayland registry")?;
    let qh = event_queue.handle();

    let compositor =
        CompositorState::bind(&globals, &qh).context("wl_compositor not available")?;
    let layer_shell =
        LayerShell::bind(&globals, &qh).context("zwlr_layer_shell_v1 not available")?;
    let shm = Shm::bind(&globals, &qh).context("wl_shm not available")?;

    let mut state = Cozy {
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        shm,
        compositor,
        layer_shell,
        qh: qh.clone(),
        surfaces: Vec::new(),
    };

    // The layer surface for each output is created from OutputHandler::new_output
    // as outputs are announced. Pump the queue until we have at least one.
    loop {
        event_queue.blocking_dispatch(&mut state)?;
        if !state.surfaces.is_empty() {
            break;
        }
    }

    loop {
        event_queue.blocking_dispatch(&mut state)?;
    }
}

/// One Background layer surface bound to a single output.
struct RainSurface {
    output: wl_output::WlOutput,
    layer: LayerSurface,
    pool: Option<SlotPool>,
    width: u32,
    height: u32,
    configured: bool,
}

/// Application state: holds the Wayland globals and one [`RainSurface`] per output.
struct Cozy {
    registry_state: RegistryState,
    output_state: OutputState,
    shm: Shm,
    compositor: CompositorState,
    layer_shell: LayerShell,
    qh: QueueHandle<Cozy>,
    surfaces: Vec<RainSurface>,
}

impl Cozy {
    /// Create a fullscreen, click-through Background layer surface for `output`.
    fn add_surface(&mut self, output: wl_output::WlOutput) {
        let surface = self.compositor.create_surface(&self.qh);

        // Empty input region => every pointer/touch event falls through to the
        // desktop behind us. This is what makes the wallpaper "non-interactive".
        let region = self.compositor.wl_compositor().create_region(&self.qh, ());
        surface.set_input_region(Some(&region));
        region.destroy();

        let layer = self.layer_shell.create_layer_surface(
            &self.qh,
            surface,
            Layer::Background,
            Some("cozy"),
            Some(&output),
        );
        layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.commit();

        self.surfaces.push(RainSurface {
            output,
            layer,
            pool: None,
            width: 0,
            height: 0,
            configured: false,
        });
    }

}

impl RainSurface {
    /// Paint the surface solid opaque black via a shared-memory buffer (M1).
    fn draw(&mut self, shm: &Shm, qh: &QueueHandle<Cozy>) {
        if self.width == 0 || self.height == 0 {
            return;
        }
        let (w, h) = (self.width as i32, self.height as i32);
        let stride = w * 4;

        let pool = self.pool.get_or_insert_with(|| {
            SlotPool::new((stride * h) as usize, shm).expect("create shm slot pool")
        });

        let (buffer, canvas) = pool
            .create_buffer(w, h, stride, wl_shm::Format::Argb8888)
            .expect("create shm buffer");

        // M1 proof-of-plumbing fill: a distinctive deep indigo so we can tell
        // cozy's surface apart from sway's (also dark) empty desktop. Replaced
        // by the wallpaper texture in M3. Little-endian ARGB8888 is [B, G, R, A].
        for px in canvas.chunks_exact_mut(4) {
            px[0] = 0x70; // B
            px[1] = 0x18; // G
            px[2] = 0x30; // R
            px[3] = 0xFF; // A
        }

        let surface = self.layer.wl_surface();
        surface.damage_buffer(0, 0, w, h);
        buffer
            .attach_to(surface)
            .expect("attach buffer to surface");

        // Schedule the next frame so we keep a steady (if static) cadence.
        surface.frame(qh, surface.clone());
        self.layer.commit();
    }
}

impl CompositorHandler for Cozy {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        let Cozy { shm, surfaces, .. } = self;
        if let Some(s) = surfaces.iter_mut().find(|s| s.layer.wl_surface() == surface) {
            s.draw(shm, qh);
        }
    }

    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for Cozy {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.add_surface(output);
    }

    fn update_output(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.surfaces.retain(|s| s.output != output);
    }
}

impl LayerShellHandler for Cozy {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        self.surfaces.retain(|s| &s.layer != layer);
    }

    fn configure(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        // Mark the whole surface opaque so the compositor can skip painting
        // anything behind it: cozy owns every pixel of the background.
        let Cozy {
            shm,
            compositor,
            surfaces,
            ..
        } = self;
        let opaque = compositor.wl_compositor().create_region(qh, ());
        if let Some(s) = surfaces.iter_mut().find(|s| &s.layer == layer) {
            let (mut w, mut h) = configure.new_size;
            // A 0 dimension means "you choose"; fall back to a sane default.
            if w == 0 {
                w = 1920;
            }
            if h == 0 {
                h = 1080;
            }
            let resized = w != s.width || h != s.height;
            s.width = w;
            s.height = h;
            if resized {
                s.pool = None; // reallocate on next draw
            }
            s.configured = true;

            opaque.add(0, 0, w as i32, h as i32);
            s.layer.wl_surface().set_opaque_region(Some(&opaque));
            opaque.destroy();

            s.draw(shm, qh);
        } else {
            opaque.destroy();
        }
    }
}

impl ShmHandler for Cozy {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

// wl_region has no events, so this dispatch is a no-op. SCTK's
// delegate_compositor! doesn't cover regions, so we provide it ourselves.
impl Dispatch<wl_region::WlRegion, ()> for Cozy {
    fn event(
        _: &mut Self,
        _: &wl_region::WlRegion,
        _: wl_region::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl ProvidesRegistryState for Cozy {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState];
}

delegate_compositor!(Cozy);
delegate_output!(Cozy);
delegate_shm!(Cozy);
delegate_layer!(Cozy);
delegate_registry!(Cozy);
