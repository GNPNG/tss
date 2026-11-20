/**
 * Procedural transmitted-light study for The Sacred Swirls.
 *
 * The animation is a function of the supplied time and audio values. It does
 * not advance an internal clock, which lets a caller render frames in any order.
 */

export const LIQUID_PALETTES = Object.freeze({
  orchid: ['#d72c95', '#9cdb27', '#048f95', '#6931ac'],
  lagoon: ['#1247ae', '#02c4d5', '#048769', '#13a0bc'],
  ember: ['#b31729', '#df4d12', '#ffc029', '#f27815'],
  twilight: ['#493d9e', '#db71ab', '#078fa8', '#7d4eb5'],
  sunrise: ['#ed5a1c', '#fdba1b', '#e8367e', '#f15e50'],
  meadow: ['#266b33', '#a4cd28', '#05a39a', '#cf9e2d'],
});

const VERTEX_SHADER = `
attribute vec2 a_position;
varying vec2 v_uv;

void main() {
  v_uv = a_position * 0.5 + 0.5;
  gl_Position = vec4(a_position, 0.0, 1.0);
}
`;

const FRAGMENT_SHADER = `
precision highp float;

varying vec2 v_uv;
uniform vec2 u_resolution;
uniform float u_time;
uniform float u_energy;
uniform float u_bass;
uniform float u_mid;
uniform float u_treble;
uniform float u_speed;
uniform float u_scale;
uniform vec3 u_color0;
uniform vec3 u_color1;
uniform vec3 u_color2;
uniform vec3 u_color3;

vec2 hash22(vec2 p) {
  p = vec2(dot(p, vec2(127.1, 311.7)), dot(p, vec2(269.5, 183.3)));
  return fract(sin(p) * 43758.5453123);
}

float noise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  float a = hash22(i).x;
  float b = hash22(i + vec2(1.0, 0.0)).x;
  float c = hash22(i + vec2(0.0, 1.0)).x;
  float d = hash22(i + vec2(1.0, 1.0)).x;
  return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

float fbm(vec2 p) {
  float value = 0.0;
  float weight = 0.5333;
  for (int i = 0; i < 4; i++) {
    value += weight * noise(p);
    p = mat2(1.62, 1.18, -1.18, 1.62) * p + 7.13;
    weight *= 0.5;
  }
  return value;
}

// Each pool's center moves continuously, and its distance field is warped by
// the surrounding pigment field.
vec3 cellField(vec2 p, float t) {
  vec2 base = floor(p);
  float nearest = 8.0;
  vec2 identity = vec2(0.0);
  for (int y = -1; y <= 1; y++) {
    for (int x = -1; x <= 1; x++) {
      vec2 cell = base + vec2(float(x), float(y));
      vec2 seed = hash22(cell);
      vec2 drift = 0.14 * sin(t * vec2(0.63, 0.49) + seed.yx * 6.28318);
      vec2 center = cell + 0.16 + 0.68 * seed + drift;
      vec2 delta = p - center;
      delta.x *= mix(0.80, 1.18, seed.x);
      delta.y *= mix(0.78, 1.14, seed.y);
      float distanceToCenter = length(delta);
      if (distanceToCenter < nearest) {
        nearest = distanceToCenter;
        identity = seed;
      }
    }
  }
  return vec3(nearest, identity);
}

vec3 palette(float value) {
  float p = fract(value) * 4.0;
  if (p < 1.0) return mix(u_color0, u_color1, smoothstep(0.0, 1.0, p));
  if (p < 2.0) return mix(u_color1, u_color2, smoothstep(0.0, 1.0, p - 1.0));
  if (p < 3.0) return mix(u_color2, u_color3, smoothstep(0.0, 1.0, p - 2.0));
  return mix(u_color3, u_color0, smoothstep(0.0, 1.0, p - 3.0));
}

void main() {
  vec2 uv = v_uv - 0.5;
  uv.x *= u_resolution.x / max(u_resolution.y, 1.0);
  float t = u_time * u_speed * 0.11;
  float activity = clamp(u_energy, 0.0, 1.0);
  vec2 p = uv * u_scale;

  vec2 flow = vec2(
    fbm(p * 0.72 + vec2(t * 0.24, -t * 0.13)),
    fbm(p * 0.72 + vec2(5.8 - t * 0.16, 8.4 + t * 0.21))
  ) - 0.5;
  p += flow * (0.95 + 0.24 * u_bass);
  p += 0.13 * vec2(
    sin(p.y * 2.0 + t * 0.4),
    cos(p.x * 1.6 - t * 0.31)
  );

  // Several warped pigment layers create filaments around the liquid pools.
  vec2 pigment = p;
  for (int i = 0; i < 3; i++) {
    float layer = float(i);
    vec2 offset = vec2(
      fbm(pigment * 1.1 + vec2(4.3 + layer, t * 0.07)),
      fbm(pigment * 1.1 + vec2(9.7, 6.1 + layer - t * 0.06))
    ) - 0.5;
    pigment += offset * 0.7;
  }

  vec3 cells = cellField(p, t);
  float dye = fbm(pigment * 1.9 + flow * 1.8 + vec2(t * 0.09, -t * 0.06));
  float dye2 = fbm(pigment * 2.8 - flow * 1.3 + vec2(15.0, 7.0));
  float radius = 0.45;
  float signedEdge = cells.x - radius + (dye2 - 0.5) * 0.18;
  float pool = 1.0 - smoothstep(-0.035, 0.04, signedEdge);
  float rim = 1.0 - smoothstep(0.004, 0.018, abs(signedEdge));
  float feather = 1.0 - smoothstep(0.04, 0.11, abs(signedEdge));
  float wash = smoothstep(0.26, 0.76, dye);
  float colorPosition = wash * 0.76 + dye2 * 0.28;
  vec3 oil = palette(colorPosition + 0.10 * dye2);
  vec3 water = palette(wash * 0.65 + 0.15 + 0.05 * u_mid);
  vec3 light = mix(water, oil, 0.3 + 0.62 * pool);

  float striationField = dye * 6.2 + dye2 * 1.6;
  float filament = 1.0 - smoothstep(0.025, 0.11, abs(fract(striationField) - 0.5));
  vec3 filamentColor = palette(colorPosition + 0.22 + 0.05 * dye2);
  light = mix(light, filamentColor * 0.66, filament * 0.36);

  // Uneven transmitted density and narrow liquid interfaces give the image
  // depth without simulating camera damage or artificial film aging.
  float density = 0.77 + 0.35 * dye;
  light *= density * (1.0 - 0.18 * rim - 0.04 * feather);
  light += palette(colorPosition + 0.14) * (0.05 * feather);
  float luminance = dot(light, vec3(0.2126, 0.7152, 0.0722));
  light = mix(vec3(luminance), light, 1.28);

  // Fine secondary droplets remain subtle compared with the larger pools.
  vec3 droplets = cellField(p * 3.7 + vec2(13.7, 5.3), t * 0.7);
  float dropletRadius = mix(0.045, 0.14, droplets.y);
  float droplet = 1.0 - smoothstep(dropletRadius, dropletRadius + 0.018, droplets.x);
  float dropletEdge = smoothstep(dropletRadius - 0.04, dropletRadius, droplets.x) * droplet;
  light = mix(light, light * 0.80 + palette(droplets.z) * 0.20, droplet * 0.5);
  light *= 1.0 - dropletEdge * 0.23;

  light *= 0.94 + activity * 0.08 + u_treble * 0.025;
  gl_FragColor = vec4(clamp(light, 0.0, 1.0), 1.0);
}
`;

function finite(value, fallback, minimum, maximum) {
  if (!Number.isFinite(value)) return fallback;
  return Math.min(maximum, Math.max(minimum, value));
}

function colorComponents(color) {
  if (Array.isArray(color) && color.length === 3 && color.every(Number.isFinite)) {
    return color.map((component) => finite(component, 0, 0, 1));
  }
  if (typeof color === 'string' && /^#[0-9a-f]{6}$/i.test(color)) {
    return [1, 3, 5].map((offset) => parseInt(color.slice(offset, offset + 2), 16) / 255);
  }
  throw new TypeError('Colors must be #rrggbb strings or arrays of three numbers from 0 to 1');
}

function compileShader(gl, type, source) {
  const shader = gl.createShader(type);
  if (!shader) throw new Error('Unable to allocate WebGL shader');
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    const error = gl.getShaderInfoLog(shader);
    gl.deleteShader(shader);
    throw new Error(`Liquid shader compilation failed: ${error}`);
  }
  return shader;
}

/** A WebGL renderer with deterministic, caller-controlled frame timing. */
export class LiquidRenderer {
  constructor(canvas) {
    if (!canvas || typeof canvas.getContext !== 'function') {
      throw new TypeError('LiquidRenderer requires a canvas');
    }
    const gl = canvas.getContext('webgl', {
      alpha: false,
      antialias: false,
      depth: false,
      stencil: false,
      preserveDrawingBuffer: true,
    });
    if (!gl) throw new Error('WebGL is unavailable');

    this.canvas = canvas;
    this.gl = gl;
    const vertex = compileShader(gl, gl.VERTEX_SHADER, VERTEX_SHADER);
    let fragment;
    let program;
    try {
      fragment = compileShader(gl, gl.FRAGMENT_SHADER, FRAGMENT_SHADER);
      program = gl.createProgram();
      if (!program) throw new Error('Unable to allocate WebGL program');
      gl.attachShader(program, vertex);
      gl.attachShader(program, fragment);
      gl.linkProgram(program);
      if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
        throw new Error(`Liquid shader linking failed: ${gl.getProgramInfoLog(program)}`);
      }
    } catch (error) {
      if (program) gl.deleteProgram(program);
      throw error;
    } finally {
      gl.deleteShader(vertex);
      if (fragment) gl.deleteShader(fragment);
    }

    const buffer = gl.createBuffer();
    if (!buffer) {
      gl.deleteProgram(program);
      throw new Error('Unable to allocate WebGL vertex buffer');
    }
    this.program = program;
    this.buffer = buffer;
    this.position = gl.getAttribLocation(program, 'a_position');
    this.uniforms = Object.fromEntries([
      'resolution', 'time', 'energy', 'bass', 'mid', 'treble', 'speed', 'scale',
      'color0', 'color1', 'color2', 'color3',
    ].map((name) => [name, gl.getUniformLocation(program, `u_${name}`)]));
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
  }

  resize(width, height) {
    if (!Number.isFinite(width) || !Number.isFinite(height) || width < 1 || height < 1) {
      throw new RangeError('Canvas dimensions must be positive finite numbers');
    }
    this.canvas.width = Math.floor(width);
    this.canvas.height = Math.floor(height);
  }

  render({ time = 0, motionTime, energy = 0, bass = 0, mid = 0, treble = 0, palette = 'orchid', speed = 1, scale = 2.2 } = {}) {
    const gl = this.gl;
    if (!this.program) throw new Error('LiquidRenderer has been disposed');
    if (gl.isContextLost()) throw new Error('WebGL context is lost');
    const colors = typeof palette === 'string' ? LIQUID_PALETTES[palette] : palette;
    if (!Array.isArray(colors) || colors.length !== 4) {
      throw new TypeError('Palette must be a known name or an array of four colors');
    }
    const components = colors.map(colorComponents);
    gl.viewport(0, 0, gl.drawingBufferWidth, gl.drawingBufferHeight);
    gl.useProgram(this.program);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.buffer);
    gl.enableVertexAttribArray(this.position);
    gl.vertexAttribPointer(this.position, 2, gl.FLOAT, false, 0, 0);
    gl.uniform2f(this.uniforms.resolution, gl.drawingBufferWidth, gl.drawingBufferHeight);
    // Callers changing speed over time can integrate it and supply motionTime,
    // avoiding a phase jump when the speed control changes.
    const usesMotionTime = Number.isFinite(motionTime);
    gl.uniform1f(this.uniforms.time, finite(usesMotionTime ? motionTime : time, 0, 0, 864000));
    gl.uniform1f(this.uniforms.energy, finite(energy, 0, 0, 1));
    gl.uniform1f(this.uniforms.bass, finite(bass, 0, 0, 1));
    gl.uniform1f(this.uniforms.mid, finite(mid, 0, 0, 1));
    gl.uniform1f(this.uniforms.treble, finite(treble, 0, 0, 1));
    gl.uniform1f(this.uniforms.speed, usesMotionTime ? 1 : finite(speed, 1, 0, 10));
    gl.uniform1f(this.uniforms.scale, finite(scale, 2.2, 0.3, 10));
    components.forEach((color, index) => gl.uniform3fv(this.uniforms[`color${index}`], color));
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
  }

  dispose() {
    if (!this.program) return;
    this.gl.deleteBuffer(this.buffer);
    this.gl.deleteProgram(this.program);
    this.buffer = null;
    this.program = null;
  }
}
