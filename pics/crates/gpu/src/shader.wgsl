// Tout est dessiné en pixels d'écran ; la conversion vers l'espace de découpe se fait ici.
// Les textures de tuiles sont en alpha droit : on prémultiplie en sortie pour mélanger avec
// le damier sans liseré sombre.

struct Globals {
    screen: vec2<f32>,
    checker_a: vec4<f32>,
    checker_b: vec4<f32>,
}

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var samp: sampler;
@group(1) @binding(0) var tile_tex: texture_2d<f32>;

struct QuadOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) screen_px: vec2<f32>,
    @location(2) color: vec4<f32>,
}

fn to_clip(p: vec2<f32>) -> vec4<f32> {
    return vec4<f32>(p.x / globals.screen.x * 2.0 - 1.0, 1.0 - p.y / globals.screen.y * 2.0, 0.0, 1.0);
}

@vertex
fn vs_quad(
    @builtin(vertex_index) vi: u32,
    @location(0) rect: vec4<f32>,
    @location(1) color: vec4<f32>,
) -> QuadOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let c = corners[vi];
    let p = mix(rect.xy, rect.zw, c);
    var out: QuadOut;
    out.position = to_clip(p);
    out.uv = c;
    out.screen_px = p;
    out.color = color;
    return out;
}

@fragment
fn fs_tile(in: QuadOut) -> @location(0) vec4<f32> {
    let c = textureSample(tile_tex, samp, in.uv);
    return vec4<f32>(c.rgb * c.a, c.a);
}

// Tuile uniforme : aucune texture, la couleur vient de l'instance (déjà prémultipliée).
@fragment
fn fs_solid(in: QuadOut) -> @location(0) vec4<f32> {
    return in.color;
}

@fragment
fn fs_checker(in: QuadOut) -> @location(0) vec4<f32> {
    let cell = floor(in.screen_px / 8.0);
    let odd = (i32(cell.x) + i32(cell.y)) & 1;
    return select(globals.checker_a, globals.checker_b, odd == 1);
}

struct LineOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_line(@location(0) pos: vec2<f32>, @location(1) color: vec4<f32>) -> LineOut {
    var out: LineOut;
    out.position = to_clip(pos);
    out.color = color;
    return out;
}

@fragment
fn fs_line(in: LineOut) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color.rgb * in.color.a, in.color.a);
}
