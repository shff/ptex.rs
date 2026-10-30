
render_mode cull_disabled, depth_test_disabled, blend_disabled;
varying vec2 mm_uv;
out vec4 mm_color;
float dot2(vec2 x) {
    return dot(x, x);
}

float rand(vec2 x) {
    return fract(cos(mod(dot(x, vec2(13.9898, 8.141)), 3.14)) * 43758.5);
}

vec2 rand2(vec2 x) {
    return fract(cos(mod(vec2(dot(x, vec2(13.9898, 8.141)),
                    dot(x, vec2(3.4562, 17.398))), vec2(3.14))) * 43758.5);
}

vec3 rand3(vec2 x) {
    return fract(cos(mod(vec3(dot(x, vec2(13.9898, 8.141)),
                    dot(x, vec2(3.4562, 17.398)),
                    dot(x, vec2(13.254, 5.867))), vec3(3.14))) * 43758.5);
}

float param_rnd(float minimum, float maximum, float seed) {
    return minimum + (maximum - minimum) * rand(vec2(seed));
}

float param_rndi(float minimum, float maximum, float seed) {
    return floor(param_rnd(minimum, maximum + 1.0, seed));
}

vec3 rgb2hsv(vec3 c) {
    vec4 K = vec4(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    vec4 p = c.g < c.b ? vec4(c.bg, K.wz) : vec4(c.gb, K.xy);
    vec4 q = c.r < p.x ? vec4(p.xyw, c.r) : vec4(c.r, p.yzx);

    float d = q.x - min(q.w, q.y);
    float e = 1.0e-10;
    return vec3(abs(q.z + (q.w - q.y) / (6.0 * d + e)), d / (q.x + e), q.x);
}

vec3 hsv2rgb(vec3 c) {
    vec4 K = vec4(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    vec3 p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
    return c.z * mix(K.xxx, clamp(p - K.xxx, 0.0, 1.0), c.y);
}
// Based on https://www.shadertoy.com/view/ldl3W8
// The MIT License
// Copyright © 2013 Inigo Quilez
vec3 iq_voronoi(vec2 x, vec2 size, vec2 stretch, float randomness, vec2 seed) {
    vec2 n = floor(x);
    vec2 f = fract(x);

    vec2 mg, mr, mc;
    float md = 8.0;
    for (int j = -1; j <= 1; j++)
        for (int i = -1; i <= 1; i++) {
            vec2 g = vec2(float(i), float(j));
            vec2 o = randomness * rand2(seed + mod(n + g + size, size));
            vec2 c = g + o;
            vec2 r = c - f;
            vec2 rr = r * stretch;
            float d = dot(rr, rr);

            if (d < md) {
                mc = c;
                md = d;
                mr = r;
                mg = g;
            }
        }

    md = 8.0;
    for (int j = -2; j <= 2; j++)
        for (int i = -2; i <= 2; i++) {
            vec2 g = mg + vec2(float(i), float(j));
            vec2 o = randomness * rand2(seed + mod(n + g + size, size));
            vec2 r = g + o - f;
            vec2 rr = (mr - r) * stretch;
            if (dot(rr, rr) > 0.00001)
                md = min(md, dot(0.5 * (mr + r) * stretch, normalize((r - mr) * stretch)));
        }

    return vec3(md, mc + n);
}

vec4 voronoi(vec2 uv, vec2 size, vec2 stretch, float intensity, float randomness, float seed) {
    uv *= size;
    vec3 v = iq_voronoi(uv, size, stretch, randomness, rand2(vec2(seed, 1.0 - seed)));
    return vec4(v.yz, intensity * length((uv - v.yz) * stretch), v.x);
}
float perlin(vec2 uv, vec2 size, int iterations, float persistence, float seed) {
    vec2 seed2 = rand2(vec2(seed, 1.0 - seed));
    float rv = 0.0;
    float coef = 1.0;
    float acc = 0.0;
    for (int i = 0; i < iterations; ++i) {
        vec2 step = vec2(1.0) / size;
        vec2 xy = floor(uv * size);
        float f0 = rand(seed2 + mod(xy, size));
        float f1 = rand(seed2 + mod(xy + vec2(1.0, 0.0), size));
        float f2 = rand(seed2 + mod(xy + vec2(0.0, 1.0), size));
        float f3 = rand(seed2 + mod(xy + vec2(1.0, 1.0), size));
        vec2 mixval = smoothstep(0.0, 1.0, fract(uv * size));
        rv += coef * mix(mix(f0, f1, mixval.x), mix(f2, f3, mixval.x), mixval.y);
        acc += coef;
        size *= 2.0;
        coef *= persistence;
    }

    return rv / acc;
}
vec3 process_normal_default(vec3 v, float multiplier) {
    return 0.5 * normalize(v.xyz * multiplier + vec3(0.0, 0.0, -1.0)) + vec3(0.5);
}

vec3 process_normal_opengl(vec3 v, float multiplier) {
    return 0.5 * normalize(v.xyz * multiplier + vec3(0.0, 0.0, 1.0)) + vec3(0.5);
}

vec3 process_normal_directx(vec3 v, float multiplier) {
    return 0.5 * normalize(v.xyz * vec3(1.0, -1.0, 1.0) * multiplier + vec3(0.0, 0.0, 1.0)) + vec3(0.5);
}
float mm_voronoi_1_output_1(vec2 uv) {
    vec4 mm_voronoi_1_uv_xyzw = voronoi(uv, vec2(4.0, 4.0), vec2(1.0, 1.0), 0.85, 1.0, 0.0);
    return mm_voronoi_1_uv_xyzw.w;
}
float mm_perlin_1_output_0(vec2 uv) {
    return perlin(uv, vec2(4.0, 4.0), int(3.0), 0.65, 0.0);
}
vec2 mm_warp_0_slope(vec2 uv, float epsilon) {
    return vec2(mm_perlin_1_output_0(fract(uv + vec2(epsilon, 0.0))) - mm_perlin_1_output_0(fract(uv - vec2(epsilon, 0.0))), mm_perlin_1_output_0(fract(uv + vec2(0.0, epsilon))) - mm_perlin_1_output_0(fract(uv - vec2(0.0, epsilon))));
}
vec4 mm_warp_0_output_0(vec2 uv) {
    vec2 mm_warp_0_uv_slope = mm_warp_0_slope(uv, 0.045);
    vec2 mm_warp_0_uv_warp = mm_warp_0_uv_slope;
    return vec4(vec3(mm_voronoi_1_output_1(uv + 0.3 * 1.0 * mm_warp_0_uv_warp)), 1.0);
}
vec4 mm_normal_map_0__buffer_output_0(vec2 uv) {
    vec2 texel = fract(uv) * 2048.0 - vec2(0.5);
    vec2 weight = fract(texel);
    return (mm_warp_0_output_0(fract((floor(texel) + vec2(0.5)) / 2048.0)) * (1.0 - weight.x) * (1.0 - weight.y) + mm_warp_0_output_0(fract((floor(texel) + vec2(1.5, 0.5)) / 2048.0)) * weight.x * (1.0 - weight.y) + mm_warp_0_output_0(fract((floor(texel) + vec2(0.5, 1.5)) / 2048.0)) * (1.0 - weight.x) * weight.y + mm_warp_0_output_0(fract((floor(texel) + vec2(1.5)) / 2048.0)) * weight.x * weight.y);
}
vec4 mm_normal_map_0__switch_output_0(vec2 uv) {
    return mm_normal_map_0__buffer_output_0(uv);
}
vec3 mm_normal_map_0__edge_detect_1_fct(vec2 uv) {
    vec3 e = vec3(1.0 / 2048.0, -1.0 / 2048.0, 0);
    vec2 rv = vec2(1.0, -1.0) * (mm_normal_map_0__switch_output_0(uv + e.xy)).r;
    rv += vec2(-1.0, 1.0) * (mm_normal_map_0__switch_output_0(uv - e.xy)).r;
    rv += vec2(1.0, 1.0) * (mm_normal_map_0__switch_output_0(uv + e.xx)).r;
    rv += vec2(-1.0, -1.0) * (mm_normal_map_0__switch_output_0(uv - e.xx)).r;
    rv += vec2(2.0, 0.0) * (mm_normal_map_0__switch_output_0(uv + e.xz)).r;
    rv += vec2(-2.0, 0.0) * (mm_normal_map_0__switch_output_0(uv - e.xz)).r;
    rv += vec2(0.0, 2.0) * (mm_normal_map_0__switch_output_0(uv + e.zx)).r;
    rv += vec2(0.0, -2.0) * (mm_normal_map_0__switch_output_0(uv - e.zx)).r;
    return vec3(rv, 0.0);
}
vec3 mm_normal_map_0__edge_detect_1_output_0(vec2 uv) {
    return process_normal_default(mm_normal_map_0__edge_detect_1_fct(uv), 0.99 * 2048.0 / 128.0);
}
vec3 mm_Material_output_7(vec2 uv) {
    return mm_normal_map_0__edge_detect_1_output_0(uv) * vec3(-1.0, 1.0, -1.0) + vec3(1.0, 0.0, 1.0);
}
void vertex() {
    vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
    mm_uv = p;
    gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
void fragment() {
    vec2 uv = mm_uv;
    mm_color = vec4((mm_Material_output_7(uv)), 1.0);
}
