//! EGL/OpenGL ES setup for a Wayland surface.
//!
//! cozy renders with OpenGL ES 3.0. To get a GL context we need EGL, and to get
//! EGL to draw into a `wl_surface` we wrap that surface in a `wl_egl_window`
//! (via [`wayland_egl::WlEglSurface`]) and hand its pointer to
//! `eglCreateWindowSurface`.
//!
//! Two types live here, split by lifetime:
//!
//! * [`Egl`] — the process-wide bits: the dynamically-loaded libEGL, the
//!   `EGLDisplay` for the Wayland connection, and a chosen framebuffer config.
//!   Created once and shared (via `Rc`) by every output's surface.
//!
//! * [`EglContext`] — the per-surface bits: the `wl_egl_window`, the
//!   `EGLSurface`, the `EGLContext`, and the [`glow`] function table used to
//!   issue GL calls. Owns its GL resources and tears them down on `Drop`.

use std::ffi::c_void;
use std::rc::Rc;

use anyhow::{anyhow, Context as _, Result};
use khronos_egl as egl;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Proxy};
use wayland_egl::WlEglSurface;

/// libEGL is loaded at runtime (the `dynamic` feature), so we never link it at
/// build time. EGL 1.4 is the floor we need — `eglGetDisplay`,
/// `eglCreateWindowSurface`, and ES context creation all live there.
type EglInstance = egl::DynamicInstance<egl::EGL1_4>;

/// Process-wide EGL state shared across all output surfaces.
pub struct Egl {
    instance: EglInstance,
    display: egl::Display,
    config: egl::Config,
}

impl Egl {
    /// Load libEGL, open the display for this Wayland connection, and pick an
    /// 8-bit RGBA window config suitable for an ES 3.0 context.
    pub fn new(conn: &Connection) -> Result<Self> {
        // SAFETY: loads libEGL.so from the system; the returned instance must
        // outlive every EGL object created through it (it owns the dlopen).
        let instance =
            unsafe { EglInstance::load_required() }.map_err(|e| anyhow!("load libEGL: {e}"))?;

        // The native display is the raw wl_display pointer behind the connection.
        let display_ptr = conn.backend().display_ptr() as *mut c_void;
        let display = unsafe { instance.get_display(display_ptr) }
            .ok_or_else(|| anyhow!("eglGetDisplay returned no display"))?;
        instance.initialize(display).context("eglInitialize")?;
        instance
            .bind_api(egl::OPENGL_ES_API)
            .context("eglBindAPI(OpenGL ES)")?;

        let config_attribs = [
            egl::SURFACE_TYPE,
            egl::WINDOW_BIT,
            egl::RENDERABLE_TYPE,
            egl::OPENGL_ES2_BIT, // also valid for ES 3.x contexts
            egl::RED_SIZE,
            8,
            egl::GREEN_SIZE,
            8,
            egl::BLUE_SIZE,
            8,
            egl::ALPHA_SIZE,
            8,
            egl::NONE,
        ];
        let config = instance
            .choose_first_config(display, &config_attribs)
            .context("eglChooseConfig")?
            .ok_or_else(|| anyhow!("no EGL config matched RGBA8 window request"))?;

        Ok(Self {
            instance,
            display,
            config,
        })
    }
}

/// Per-surface GL context: a `wl_egl_window`, its `EGLSurface`/`EGLContext`, and
/// the [`glow`] entry points. Drawing goes through [`make_current`] →
/// GL calls → [`swap_buffers`].
///
/// [`make_current`]: EglContext::make_current
/// [`swap_buffers`]: EglContext::swap_buffers
pub struct EglContext {
    egl: Rc<Egl>,
    /// Keeps the `wl_egl_window` alive for as long as the `EGLSurface`; resized
    /// on output configure. Never read directly after construction.
    _wl_egl_surface: WlEglSurface,
    surface: egl::Surface,
    context: egl::Context,
    /// The GL function table. Public so renderers can issue draw calls.
    pub gl: glow::Context,
}

impl EglContext {
    /// Wrap `wl_surface` in a `wl_egl_window` of size `w`×`h` and create an
    /// ES 3.0 context bound to it. The context is left current.
    pub fn new(egl: Rc<Egl>, wl_surface: &WlSurface, w: i32, h: i32) -> Result<Self> {
        let wl_egl_surface =
            WlEglSurface::new(wl_surface.id(), w, h).context("create wl_egl_window")?;

        // SAFETY: the wl_egl_window pointer is valid for the lifetime of
        // `wl_egl_surface`, which we store alongside the EGLSurface below.
        let surface = unsafe {
            egl.instance.create_window_surface(
                egl.display,
                egl.config,
                wl_egl_surface.ptr() as egl::NativeWindowType,
                None,
            )
        }
        .context("eglCreateWindowSurface")?;

        let context_attribs = [
            egl::CONTEXT_MAJOR_VERSION,
            3,
            egl::CONTEXT_MINOR_VERSION,
            0,
            egl::NONE,
        ];
        let context = egl
            .instance
            .create_context(egl.display, egl.config, None, &context_attribs)
            .context("eglCreateContext (ES 3.0)")?;

        egl.instance
            .make_current(egl.display, Some(surface), Some(surface), Some(context))
            .context("eglMakeCurrent")?;

        // SAFETY: a context is current, so eglGetProcAddress can resolve GL
        // entry points; glow only calls these while a context is current.
        let gl = unsafe {
            glow::Context::from_loader_function(|name| {
                egl.instance
                    .get_proc_address(name)
                    .map_or(std::ptr::null(), |p| p as *const c_void)
            })
        };

        Ok(Self {
            egl,
            _wl_egl_surface: wl_egl_surface,
            surface,
            context,
            gl,
        })
    }

    /// Make this surface's context current on the calling thread.
    pub fn make_current(&self) -> Result<()> {
        self.egl
            .instance
            .make_current(
                self.egl.display,
                Some(self.surface),
                Some(self.surface),
                Some(self.context),
            )
            .context("eglMakeCurrent")
    }

    /// Resize the backing `wl_egl_window` after an output configure.
    pub fn resize(&self, w: i32, h: i32) {
        self._wl_egl_surface.resize(w, h, 0, 0);
    }

    /// Present the rendered frame. On the Mesa Wayland platform this also
    /// commits the `wl_surface`, so any frame callback queued beforehand rides
    /// along with the commit.
    pub fn swap_buffers(&self) -> Result<()> {
        self.egl
            .instance
            .swap_buffers(self.egl.display, self.surface)
            .context("eglSwapBuffers")
    }
}

impl Drop for EglContext {
    fn drop(&mut self) {
        // Best-effort teardown: detach the context, then destroy both objects.
        let _ = self
            .egl
            .instance
            .make_current(self.egl.display, None, None, None);
        let _ = self
            .egl
            .instance
            .destroy_surface(self.egl.display, self.surface);
        let _ = self
            .egl
            .instance
            .destroy_context(self.egl.display, self.context);
    }
}
