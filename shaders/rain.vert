#version 300 es

// Fullscreen-triangle vertex shader.
//
// No vertex buffer is bound: the three positions are derived from gl_VertexID,
// forming one oversized triangle that covers the whole viewport. We emit v_uv
// in [0,1] with the origin at the TOP-LEFT of the screen, matching image data
// uploaded top-row-first (so screen-top samples the wallpaper's top).

out vec2 v_uv;

void main() {
    // id 0 -> (-1,-1), id 1 -> (3,-1), id 2 -> (-1,3)
    vec2 pos = vec2(
        float((gl_VertexID & 1) << 2) - 1.0,
        float((gl_VertexID & 2) << 1) - 1.0
    );
    v_uv = vec2(pos.x * 0.5 + 0.5, 0.5 - pos.y * 0.5);
    gl_Position = vec4(pos, 0.0, 1.0);
}
