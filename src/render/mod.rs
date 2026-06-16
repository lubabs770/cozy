//! Rendering layer: EGL/GLES context management and (later) the rain shader
//! pipeline.
//!
//! For now this is just [`egl`], which brings up an OpenGL ES 3.0 context on a
//! Wayland surface. M3 adds wallpaper texture upload and the fullscreen draw;
//! M4 adds the rain effects.

pub mod egl;
pub mod gl;
pub mod texture;
