
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
	return minimum+(maximum-minimum)*rand(vec2(seed));
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
float perlin(vec2 uv, vec2 size, int iterations, float persistence, float seed) {
	vec2 seed2 = rand2(vec2(seed, 1.0-seed));
	float rv = 0.0;
	float coef = 1.0;
	float acc = 0.0;
	for (int i = 0; i < iterations; ++i) {
		vec2 step = vec2(1.0)/size;
		vec2 xy = floor(uv*size);
		float f0 = rand(seed2+mod(xy, size));
		float f1 = rand(seed2+mod(xy+vec2(1.0, 0.0), size));
		float f2 = rand(seed2+mod(xy+vec2(0.0, 1.0), size));
		float f3 = rand(seed2+mod(xy+vec2(1.0, 1.0), size));
		vec2 mixval = smoothstep(0.0, 1.0, fract(uv*size));
		rv += coef * mix(mix(f0, f1, mixval.x), mix(f2, f3, mixval.x), mixval.y);
		acc += coef;
		size *= 2.0;
		coef *= persistence;
	}
	
	return rv / acc;
}
float mm_perlin_0_output_0(vec2 uv) {

return perlin(uv, vec2(4.0, 4.0), int(6.0), 0.85, 0.0);
}
vec4 mm_colorize_2_output_0(vec2 uv) {

return ((mm_perlin_0_output_0(uv)) < 0.0 ? vec4(0.364583, 0.364583, 0.364583, 1.0) : ((mm_perlin_0_output_0(uv)) < 1.0 ? mix(vec4(0.364583, 0.364583, 0.364583, 1.0), vec4(1.0, 1.0, 1.0, 1.0), clamp(((mm_perlin_0_output_0(uv))-0.0)/1.0, 0.0, 1.0)) : vec4(1.0, 1.0, 1.0, 1.0)));
}
float mm_Material_output_13(vec2 uv) {

return (mm_colorize_2_output_0(uv)).r;
}
void vertex() {
    vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
    mm_uv = p;
    gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
void fragment() {
    vec2 uv = mm_uv;
    mm_color = vec4((mm_Material_output_13(uv)), (mm_Material_output_13(uv)), (mm_Material_output_13(uv)), 1.0);
}
