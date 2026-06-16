//! The GL renderer: shader program, fullscreen-triangle draw, and uniforms.
//!
//! One [`Renderer`] lives per surface (GL objects belong to a single context).
//! The shaders are embedded at build time via `include_str!` so a release build
//! is a single self-contained binary; iterating on them still only needs a
//! rebuild, not a code change.

use anyhow::{anyhow, Result};
use glow::HasContext as _;

use super::texture::Texture;

const VERT_SRC: &str = include_str!("../../shaders/rain.vert");
const FRAG_SRC: &str = include_str!("../../shaders/rain.frag");

/// Compiled program, the (attribute-less) VAO required by ES 3.0 for
/// `draw_arrays`, the wallpaper texture, and cached uniform locations.
pub struct Renderer {
    program: glow::Program,
    vao: glow::VertexArray,
    wallpaper: Texture,
    u_resolution: Option<glow::UniformLocation>,
    u_tex_resolution: Option<glow::UniformLocation>,
    u_wallpaper: Option<glow::UniformLocation>,
    u_time: Option<glow::UniformLocation>,
}

impl Renderer {
    /// Build the program and upload the wallpaper. Requires a current context.
    pub fn new(gl: &glow::Context, wallpaper_bytes: &[u8]) -> Result<Self> {
        let wallpaper = Texture::from_bytes(gl, wallpaper_bytes)?;
        // SAFETY: a GL context is current for the lifetime of these calls.
        unsafe {
            let program = link_program(gl, VERT_SRC, FRAG_SRC)?;
            let vao = gl
                .create_vertex_array()
                .map_err(|e| anyhow!("create VAO: {e}"))?;
            Ok(Self {
                u_resolution: gl.get_uniform_location(program, "u_resolution"),
                u_tex_resolution: gl.get_uniform_location(program, "u_tex_resolution"),
                u_wallpaper: gl.get_uniform_location(program, "u_wallpaper"),
                u_time: gl.get_uniform_location(program, "u_time"),
                program,
                vao,
                wallpaper,
            })
        }
    }

    /// Draw one frame into the current framebuffer at `width`×`height`.
    /// `time` is seconds since startup and drives the animated effects.
    pub fn draw(&self, gl: &glow::Context, width: i32, height: i32, time: f32) {
        // SAFETY: a GL context is current; all handles were created from it.
        unsafe {
            gl.viewport(0, 0, width, height);
            gl.use_program(Some(self.program));
            gl.bind_vertex_array(Some(self.vao));

            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.wallpaper.handle));
            gl.uniform_1_i32(self.u_wallpaper.as_ref(), 0);
            gl.uniform_2_f32(self.u_resolution.as_ref(), width as f32, height as f32);
            gl.uniform_2_f32(
                self.u_tex_resolution.as_ref(),
                self.wallpaper.width as f32,
                self.wallpaper.height as f32,
            );
            gl.uniform_1_f32(self.u_time.as_ref(), time);

            gl.draw_arrays(glow::TRIANGLES, 0, 3);
        }
    }
}

/// Compile the vertex + fragment sources and link them into a program,
/// surfacing the GL info log on any failure.
///
/// # Safety
/// A GL context must be current.
unsafe fn link_program(gl: &glow::Context, vert: &str, frag: &str) -> Result<glow::Program> {
    let program = gl
        .create_program()
        .map_err(|e| anyhow!("create program: {e}"))?;

    let mut shaders = Vec::with_capacity(2);
    for (stage, src) in [(glow::VERTEX_SHADER, vert), (glow::FRAGMENT_SHADER, frag)] {
        let shader = gl
            .create_shader(stage)
            .map_err(|e| anyhow!("create shader: {e}"))?;
        gl.shader_source(shader, src);
        gl.compile_shader(shader);
        if !gl.get_shader_compile_status(shader) {
            return Err(anyhow!(
                "shader compile failed: {}",
                gl.get_shader_info_log(shader)
            ));
        }
        gl.attach_shader(program, shader);
        shaders.push(shader);
    }

    gl.link_program(program);
    if !gl.get_program_link_status(program) {
        return Err(anyhow!(
            "program link failed: {}",
            gl.get_program_info_log(program)
        ));
    }

    // Shaders are no longer needed once linked.
    for shader in shaders {
        gl.detach_shader(program, shader);
        gl.delete_shader(shader);
    }

    Ok(program)
}
