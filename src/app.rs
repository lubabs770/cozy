//! Application state and Wayland event handling.
//!
//! [`Cozy`] holds the Wayland globals and one [`RainSurface`] per output, and
//! implements all the smithay-client-toolkit handler traits that drive the
//! client: registry, outputs, compositor (frame callbacks), and layer shell
//! (configure / close).

use std::rc::Rc;
use std::time::Instant;

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry,
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
};
use wayland_client::{
    protocol::{wl_output, wl_region, wl_surface},
    Connection, Dispatch, QueueHandle,
};

use crate::render::egl::Egl;
use crate::surface::RainSurface;

/// Fallback dimensions when the compositor leaves the size up to us (a configure
/// with a zero width or height). Real outputs send concrete sizes.
const FALLBACK_SIZE: (u32, u32) = (1920, 1080);

/// Top-level client state: Wayland globals plus one [`RainSurface`] per output.
pub struct Cozy {
    registry_state: RegistryState,
    output_state: OutputState,
    compositor: CompositorState,
    layer_shell: LayerShell,
    /// Shared, process-wide EGL state; each surface builds its own context from it.
    egl: Rc<Egl>,
    /// Encoded wallpaper image; each surface decodes/uploads it on first draw.
    wallpaper: &'static [u8],
    /// Start time; the elapsed seconds become the shader's `u_time`.
    start: Instant,
    qh: QueueHandle<Cozy>,
    surfaces: Vec<RainSurface>,
}

impl Cozy {
    pub fn new(
        registry_state: RegistryState,
        output_state: OutputState,
        compositor: CompositorState,
        layer_shell: LayerShell,
        egl: Rc<Egl>,
        wallpaper: &'static [u8],
        qh: QueueHandle<Cozy>,
    ) -> Self {
        Self {
            registry_state,
            output_state,
            compositor,
            layer_shell,
            egl,
            wallpaper,
            start: Instant::now(),
            qh,
            surfaces: Vec::new(),
        }
    }

    /// Whether any surface has been created yet (used to gate the startup loop).
    pub fn has_surfaces(&self) -> bool {
        !self.surfaces.is_empty()
    }

    /// Create a fullscreen, click-through Background layer surface for `output`.
    fn add_surface(&mut self, output: wl_output::WlOutput) {
        let surface = self.compositor.create_surface(&self.qh);

        // Empty input region => every pointer/touch event falls through to the
        // desktop behind us. This is what makes the wallpaper non-interactive.
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

        self.surfaces.push(RainSurface::new(output, layer));
    }

    /// Draw a surface, logging (rather than panicking on) any GL/EGL error.
    fn draw_surface(&mut self, index: usize) {
        let time = self.start.elapsed().as_secs_f32();
        let Cozy {
            egl,
            wallpaper,
            surfaces,
            qh,
            ..
        } = self;
        if let Some(s) = surfaces.get_mut(index) {
            if let Err(e) = s.draw(egl, wallpaper, time, qh) {
                eprintln!("cozy: draw error: {e:#}");
            }
        }
    }

    fn index_of_surface(&self, surface: &wl_surface::WlSurface) -> Option<usize> {
        self.surfaces
            .iter()
            .position(|s| s.layer.wl_surface() == surface)
    }

    fn index_of_layer(&self, layer: &LayerSurface) -> Option<usize> {
        self.surfaces.iter().position(|s| &s.layer == layer)
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
        _: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        if let Some(i) = self.index_of_surface(surface) {
            self.draw_surface(i);
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

    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, output: wl_output::WlOutput) {
        self.add_surface(output);
    }

    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

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
        let Some(index) = self.index_of_layer(layer) else {
            return;
        };

        let (mut w, mut h) = configure.new_size;
        if w == 0 {
            w = FALLBACK_SIZE.0;
        }
        if h == 0 {
            h = FALLBACK_SIZE.1;
        }
        self.surfaces[index].set_size(w, h);

        // Mark the whole surface opaque so the compositor can skip painting
        // anything behind it: cozy owns every pixel of the background.
        let opaque = self.compositor.wl_compositor().create_region(qh, ());
        opaque.add(0, 0, w as i32, h as i32);
        self.surfaces[index]
            .layer
            .wl_surface()
            .set_opaque_region(Some(&opaque));
        opaque.destroy();

        self.draw_surface(index);
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
delegate_layer!(Cozy);
delegate_registry!(Cozy);
