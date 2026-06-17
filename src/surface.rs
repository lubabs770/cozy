//! One Background layer surface per output, plus its GL drawing.
//!
//! A [`RainSurface`] owns the `wlr-layer-shell` surface for a single output and
//! the [`EglContext`] used to render into it. The EGL context is created lazily
//! on the first configure (when the output's size is known) and resized on
//! subsequent configures.

use std::rc::Rc;

use anyhow::Result;
use smithay_client_toolkit::shell::{wlr_layer::LayerSurface, WaylandSurface};
use wayland_client::protocol::wl_output;
use wayland_client::QueueHandle;

use crate::app::Cozy;
use crate::render::egl::{Egl, EglContext};
use crate::render::gl::Renderer;

/// A fullscreen, click-through Background layer surface bound to one output.
pub struct RainSurface {
    /// The output this surface is pinned to (used for hotplug removal in M5).
    pub output: wl_output::WlOutput,
    /// The layer-shell surface; also our handle to the underlying `wl_surface`.
    pub layer: LayerSurface,
    /// GL context, created on first configure once we know the size.
    egl: Option<EglContext>,
    /// GL renderer, created once the context exists (per-context resource).
    renderer: Option<Renderer>,
    width: u32,
    height: u32,
    /// The wallpaper generation this surface's texture currently holds; when it
    /// trails [`Cozy::wallpaper_gen`] the texture is re-uploaded on next draw.
    ///
    /// [`Cozy::wallpaper_gen`]: crate::app::Cozy
    last_gen: u64,
    /// Set once the compositor has sent its first configure.
    pub configured: bool,
}

impl RainSurface {
    /// Create a surface record. The EGL context is deferred until [`draw`] runs
    /// with a non-zero size.
    ///
    /// [`draw`]: RainSurface::draw
    pub fn new(output: wl_output::WlOutput, layer: LayerSurface) -> Self {
        Self {
            output,
            layer,
            egl: None,
            renderer: None,
            width: 0,
            height: 0,
            last_gen: 0,
            configured: false,
        }
    }

    /// Record a new size from a configure event, returning whether it changed.
    pub fn set_size(&mut self, width: u32, height: u32) -> bool {
        let changed = width != self.width || height != self.height;
        self.width = width;
        self.height = height;
        self.configured = true;
        changed
    }

    /// Render one frame: ensure the GL context and renderer exist at the current
    /// size, (re-)upload the wallpaper if it changed, draw, queue the next frame
    /// callback, and present.
    ///
    /// `wallpaper` is the current encoded image bytes and `gen` its generation
    /// counter; when `gen` advances past what this surface last uploaded, the
    /// texture is rebuilt (otherwise the bytes are only decoded once).
    pub fn draw(
        &mut self,
        egl: &Rc<Egl>,
        wallpaper: &[u8],
        gen: u64,
        time: f32,
        qh: &QueueHandle<Cozy>,
    ) -> Result<()> {
        if self.width == 0 || self.height == 0 {
            return Ok(());
        }
        let (w, h) = (self.width as i32, self.height as i32);
        // Cheap proxy clone so we don't hold a borrow of `self.layer` while we
        // mutate `self.egl` / `self.renderer` below.
        let wl_surface = self.layer.wl_surface().clone();

        // Create the GL context on first use, then keep its window sized to the
        // surface (resize is a no-op on the just-created one).
        if self.egl.is_none() {
            self.egl = Some(EglContext::new(egl.clone(), &wl_surface, w, h)?);
        }
        let ctx = self.egl.as_ref().expect("egl context initialized above");
        ctx.resize(w, h);
        ctx.make_current()?;

        // Build the renderer lazily; thereafter re-upload only when the wallpaper
        // generation advances (a `set` command landed).
        if self.renderer.is_none() {
            self.renderer = Some(Renderer::new(&ctx.gl, wallpaper)?);
            self.last_gen = gen;
        } else if gen != self.last_gen {
            if let Some(r) = self.renderer.as_mut() {
                r.set_wallpaper(&ctx.gl, wallpaper)?;
            }
            self.last_gen = gen;
        }
        let renderer = self.renderer.as_ref().expect("renderer initialized above");
        renderer.draw(&ctx.gl, w, h, time);

        // Queue the next frame before presenting; swap_buffers commits the
        // surface, carrying this callback request with it.
        wl_surface.frame(qh, wl_surface.clone());
        ctx.swap_buffers()?;
        Ok(())
    }
}
