//! The GL renderer: a registry of effect shaders, the fullscreen-triangle draw,
//! and uniforms.
//!
//! One [`Renderer`] lives per surface (GL objects belong to a single context).
//! cozy renders exactly one **effect** at a time — a fragment shader honouring a
//! shared uniform contract (`u_resolution`, `u_tex_resolution`, `u_wallpaper`,
//! `u_time`, `u_wind`, `u_intensity`). Effects are registered in [`EFFECTS`] and
//! switched live with [`Renderer::set_effect`]; adding one is a new shader file
//! plus one table entry.
//!
//! Shaders are embedded at build time via `include_str!` so a release build is a
//! single self-contained binary; iterating on them still only needs a rebuild.

use anyhow::{anyhow, Result};
use glow::HasContext as _;

use super::texture::Texture;

const VERT_SRC: &str = include_str!("../../shaders/rain.vert");

/// All built-in effects, by name. The first entry is the default.
///
/// Note: `droplet` is ported from "Heartfelt" by BigWings and is licensed
/// CC BY-NC-SA 3.0 (see the shader header), unlike the rest of cozy (MIT).
/// All other effects are hand-built and MIT.
/// All built-in effects as `(name, description, fragment source)`. The
/// description is a one-line blurb for `cozy effect` / `--help` listings.
const EFFECTS: &[(&str, &str, &str)] = &[
    (
        "droplet",
        "rain on glass, refracting the wallpaper (Heartfelt)",
        include_str!("../../shaders/effects/droplet.frag"),
    ),
    (
        "classic",
        "slanted streaks with running glass droplets",
        include_str!("../../shaders/effects/classic.frag"),
    ),
    (
        "pouring",
        "heavy downpour with fog and large drops",
        include_str!("../../shaders/effects/pouring.frag"),
    ),
    (
        "ripple",
        "rain on a water surface, expanding rings",
        include_str!("../../shaders/effects/ripple.frag"),
    ),
    (
        "snow",
        "softly drifting snowflakes",
        include_str!("../../shaders/effects/snow.frag"),
    ),
    (
        "sleet",
        "fast icy pellets with diagonal streaks",
        include_str!("../../shaders/effects/sleet.frag"),
    ),
];

/// The effect cozy starts with when none is requested.
pub const DEFAULT_EFFECT: &str = EFFECTS[0].0;

/// Look up an effect's fragment source by name.
fn effect_source(name: &str) -> Result<&'static str> {
    EFFECTS
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, _, src)| *src)
        .ok_or_else(|| anyhow!("unknown effect: {name:?} (known: {})", effect_names()))
}

/// Whether `name` is a registered effect (used to validate control commands).
pub fn effect_exists(name: &str) -> bool {
    EFFECTS.iter().any(|(n, _, _)| *n == name)
}

/// Comma-separated list of effect names, for error/help messages.
pub fn effect_names() -> String {
    EFFECTS
        .iter()
        .map(|(n, _, _)| *n)
        .collect::<Vec<_>>()
        .join(", ")
}

/// All effects as `(name, description)` pairs, for help/listing output.
pub fn effect_descriptions() -> impl Iterator<Item = (&'static str, &'static str)> {
    EFFECTS.iter().map(|(n, d, _)| (*n, *d))
}

/// Cached uniform locations for the currently linked program. Unused uniforms
/// resolve to `None` and are simply skipped at draw time, so an effect need only
/// declare the uniforms it actually uses.
#[derive(Default)]
struct Uniforms {
    resolution: Option<glow::UniformLocation>,
    tex_resolution: Option<glow::UniformLocation>,
    wallpaper: Option<glow::UniformLocation>,
    time: Option<glow::UniformLocation>,
    wind: Option<glow::UniformLocation>,
    intensity: Option<glow::UniformLocation>,
    overlay: Option<glow::UniformLocation>,
}

impl Uniforms {
    fn locate(gl: &glow::Context, program: glow::Program) -> Self {
        // SAFETY: a GL context is current and `program` was linked from it.
        unsafe {
            Self {
                resolution: gl.get_uniform_location(program, "u_resolution"),
                tex_resolution: gl.get_uniform_location(program, "u_tex_resolution"),
                wallpaper: gl.get_uniform_location(program, "u_wallpaper"),
                time: gl.get_uniform_location(program, "u_time"),
                wind: gl.get_uniform_location(program, "u_wind"),
                intensity: gl.get_uniform_location(program, "u_intensity"),
                overlay: gl.get_uniform_location(program, "u_overlay"),
            }
        }
    }
}

/// Compiled program for the current effect, the (attribute-less) VAO required by
/// ES 3.0 for `draw_arrays`, the wallpaper texture, and cached uniform locations.
pub struct Renderer {
    program: glow::Program,
    vao: glow::VertexArray,
    wallpaper: Texture,
    current_effect: String,
    uniforms: Uniforms,
}

impl Renderer {
    /// Build the program for `effect` and upload the wallpaper. Requires a
    /// current context.
    pub fn new(gl: &glow::Context, wallpaper_bytes: &[u8], effect: &str) -> Result<Self> {
        let wallpaper = Texture::from_bytes(gl, wallpaper_bytes)?;
        let src = effect_source(effect)?;
        // SAFETY: a GL context is current for the lifetime of these calls.
        let (program, vao) = unsafe {
            let program = link_program(gl, VERT_SRC, src)?;
            let vao = gl
                .create_vertex_array()
                .map_err(|e| anyhow!("create VAO: {e}"))?;
            (program, vao)
        };
        Ok(Self {
            uniforms: Uniforms::locate(gl, program),
            program,
            vao,
            wallpaper,
            current_effect: effect.to_string(),
        })
    }

    /// Switch to another effect, recompiling its program and freeing the old
    /// one. No-op if already current. Requires a current context.
    pub fn set_effect(&mut self, gl: &glow::Context, effect: &str) -> Result<()> {
        if effect == self.current_effect {
            return Ok(());
        }
        let src = effect_source(effect)?;
        // SAFETY: a GL context is current; old program belongs to it.
        let program = unsafe { link_program(gl, VERT_SRC, src)? };
        unsafe { gl.delete_program(self.program) };
        self.program = program;
        self.uniforms = Uniforms::locate(gl, program);
        self.current_effect = effect.to_string();
        Ok(())
    }

    /// Replace the wallpaper texture with a freshly decoded one (used when a
    /// `set` command swaps the wallpaper at runtime). The old texture is freed.
    /// Requires a current context. Dimensions may differ from the previous
    /// wallpaper; `u_tex_resolution` is read from the new texture at draw time,
    /// so cover-fit stays correct.
    pub fn set_wallpaper(&mut self, gl: &glow::Context, wallpaper_bytes: &[u8]) -> Result<()> {
        let new = Texture::from_bytes(gl, wallpaper_bytes)?;
        self.wallpaper.delete(gl);
        self.wallpaper = new;
        Ok(())
    }

    /// Draw one frame into the current framebuffer at `width`×`height`.
    /// `time` is seconds since startup; `wind`/`intensity` are the (future
    /// weather-driven) effect parameters; `overlay` selects premultiplied-alpha
    /// output for transparent compositing.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        gl: &glow::Context,
        width: i32,
        height: i32,
        time: f32,
        wind: f32,
        intensity: f32,
        overlay: bool,
    ) {
        // SAFETY: a GL context is current; all handles were created from it.
        unsafe {
            gl.viewport(0, 0, width, height);
            gl.use_program(Some(self.program));
            gl.bind_vertex_array(Some(self.vao));

            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.wallpaper.handle));
            gl.uniform_1_i32(self.uniforms.wallpaper.as_ref(), 0);
            gl.uniform_2_f32(
                self.uniforms.resolution.as_ref(),
                width as f32,
                height as f32,
            );
            gl.uniform_2_f32(
                self.uniforms.tex_resolution.as_ref(),
                self.wallpaper.width as f32,
                self.wallpaper.height as f32,
            );
            gl.uniform_1_f32(self.uniforms.time.as_ref(), time);
            gl.uniform_1_f32(self.uniforms.wind.as_ref(), wind);
            gl.uniform_1_f32(self.uniforms.intensity.as_ref(), intensity);
            gl.uniform_1_i32(self.uniforms.overlay.as_ref(), overlay as i32);

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
