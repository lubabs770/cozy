//! Application state and Wayland event handling.
//!
//! [`Cozy`] holds the Wayland globals and one [`RainSurface`] per output, and
//! implements all the smithay-client-toolkit handler traits that drive the
//! client: registry, outputs, compositor (frame callbacks), and layer shell
//! (configure / close).

use std::rc::Rc;
use std::sync::mpsc::Receiver;
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

use crate::control::Command;
use crate::render::egl::Egl;
use crate::render::gl;
use crate::surface::{FrameParams, RainSurface};

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
    /// Encoded wallpaper image; each surface decodes/uploads it lazily. Replaced
    /// wholesale when a `set` command arrives over the control socket.
    wallpaper: Vec<u8>,
    /// Bumped every time `wallpaper` changes; surfaces compare it to know when to
    /// re-upload their texture rather than re-decoding every frame.
    wallpaper_gen: u64,
    /// The active rain effect (a shader name from the renderer's registry).
    effect: String,
    /// Bumped every time `effect` changes; surfaces compare it to know when to
    /// recompile their program.
    effect_gen: u64,
    /// Weather-driven shader parameters: horizontal wind skew and rain intensity
    /// (0..1). Defaults until a weather source drives them in a later phase.
    wind: f32,
    intensity: f32,
    /// Run as a transparent overlay above an external wallpaper daemon instead
    /// of owning the wallpaper. Changes the layer (Bottom, not Background), drops
    /// the opaque region, and tells shaders to output premultiplied alpha.
    overlay: bool,
    /// Control commands from the socket listener thread, drained each frame.
    commands: Receiver<Command>,
    /// Start time; the elapsed seconds become the shader's `u_time`.
    start: Instant,
    qh: QueueHandle<Cozy>,
    surfaces: Vec<RainSurface>,
}

impl Cozy {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        registry_state: RegistryState,
        output_state: OutputState,
        compositor: CompositorState,
        layer_shell: LayerShell,
        egl: Rc<Egl>,
        wallpaper: Vec<u8>,
        overlay: bool,
        commands: Receiver<Command>,
        qh: QueueHandle<Cozy>,
    ) -> Self {
        Self {
            registry_state,
            output_state,
            compositor,
            layer_shell,
            egl,
            wallpaper,
            wallpaper_gen: 0,
            effect: gl::DEFAULT_EFFECT.to_string(),
            effect_gen: 0,
            wind: 0.0,
            intensity: 0.7,
            overlay,
            commands,
            start: Instant::now(),
            qh,
            surfaces: Vec::new(),
        }
    }

    /// Drain any pending control commands. Called once per frame; the animated
    /// rain keeps frames flowing, so commands apply within ~one frame.
    fn pump_commands(&mut self) {
        while let Ok(cmd) = self.commands.try_recv() {
            match cmd {
                Command::SetWallpaper { path } => match std::fs::read(&path) {
                    Ok(bytes) => {
                        self.wallpaper = bytes;
                        self.wallpaper_gen = self.wallpaper_gen.wrapping_add(1);
                    }
                    Err(e) => eprintln!("cozy: set wallpaper {}: {e}", path.display()),
                },
                Command::SetEffect { name } => {
                    if gl::effect_exists(&name) {
                        self.effect = name;
                        self.effect_gen = self.effect_gen.wrapping_add(1);
                    } else {
                        eprintln!(
                            "cozy: unknown effect {name:?} (known: {})",
                            gl::effect_names()
                        );
                    }
                }
                Command::SetWeather { wind, precip } => {
                    self.wind = wind;
                    self.intensity = precip.clamp(0.0, 1.0);
                }
            }
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

        // Overlay mode sits on the Bottom layer, just above the wallpaper drawn
        // by an external daemon; opaque mode owns the Background layer itself.
        let layer_kind = if self.overlay {
            Layer::Bottom
        } else {
            Layer::Background
        };
        let layer = self.layer_shell.create_layer_surface(
            &self.qh,
            surface,
            layer_kind,
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
            wallpaper_gen,
            effect,
            effect_gen,
            wind,
            intensity,
            overlay,
            surfaces,
            qh,
            ..
        } = self;
        let params = FrameParams {
            wallpaper: wallpaper.as_slice(),
            wallpaper_gen: *wallpaper_gen,
            effect: effect.as_str(),
            effect_gen: *effect_gen,
            time,
            wind: *wind,
            intensity: *intensity,
            overlay: *overlay,
        };
        if let Some(s) = surfaces.get_mut(index) {
            if let Err(e) = s.draw(egl, &params, qh) {
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
        self.pump_commands();
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

        // In opaque mode, mark the whole surface opaque so the compositor can
        // skip painting anything behind it: cozy owns every pixel. In overlay
        // mode we must NOT do this — the compositor has to alpha-blend cozy over
        // the external wallpaper showing through our transparent pixels.
        if !self.overlay {
            let opaque = self.compositor.wl_compositor().create_region(qh, ());
            opaque.add(0, 0, w as i32, h as i32);
            self.surfaces[index]
                .layer
                .wl_surface()
                .set_opaque_region(Some(&opaque));
            opaque.destroy();
        }

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
