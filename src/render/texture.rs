//! Decode an image and upload it as an OpenGL ES 2D texture.

use anyhow::{anyhow, Context as _, Result};
use glow::{HasContext as _, PixelUnpackData};

/// A GL texture plus its source dimensions (needed for aspect-correct sampling).
pub struct Texture {
    pub handle: glow::Texture,
    pub width: u32,
    pub height: u32,
}

impl Texture {
    /// Decode `bytes` (any format the `image` crate is built with) and upload it
    /// as an `RGBA8` texture with linear filtering and edge clamping.
    ///
    /// Must be called with a GL context current. The image's top row is uploaded
    /// first, so texture coordinate `t = 0` corresponds to the top of the image.
    pub fn from_bytes(gl: &glow::Context, bytes: &[u8]) -> Result<Self> {
        let img = image::load_from_memory(bytes)
            .context("decode image")?
            .to_rgba8();
        let (width, height) = img.dimensions();
        let pixels = img.into_raw();

        // SAFETY: a GL context is current; the slice is exactly width*height*4
        // bytes of RGBA, matching the format/type passed to tex_image_2d.
        let handle = unsafe {
            let handle = gl
                .create_texture()
                .map_err(|e| anyhow!("create texture: {e}"))?;
            gl.bind_texture(glow::TEXTURE_2D, Some(handle));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                width as i32,
                height as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                PixelUnpackData::Slice(Some(pixels.as_slice())),
            );
            // Mipmaps + trilinear minification let shaders sample blurred
            // versions of the wallpaper via `textureLod` — the "droplet" effect
            // uses this for through-the-glass depth of field.
            gl.generate_mipmap(glow::TEXTURE_2D);
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR_MIPMAP_LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );
            handle
        };

        Ok(Self {
            handle,
            width,
            height,
        })
    }

    /// Free the underlying GL texture. Call before dropping the last reference
    /// when replacing a texture, so swapping wallpapers doesn't leak one per
    /// change. Requires the owning GL context to be current.
    pub fn delete(&self, gl: &glow::Context) {
        // SAFETY: a GL context is current and `handle` was created from it.
        unsafe { gl.delete_texture(self.handle) };
    }
}
