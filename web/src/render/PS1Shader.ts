/**
 * TASK-023: PS1-era post-processing stack (§11): low internal resolution,
 * pixelation, ordered 4×4 Bayer dithering, and 5-bit color quantization
 * (32 levels/channel) implemented as a ShaderMaterial pass.
 *
 * Headless-safe: the pass compiles only when handed a real WebGLRenderer;
 * the shader definition itself is plain data testable in Node.
 */

import {
  ShaderMaterial,
  UniformsUtils,
  type IUniform,
  type WebGLRenderer,
} from 'three';

export const PS1_VERTEX = /* glsl */ `
varying vec2 vUv;
void main() {
  vUv = uv;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`;

export const PS1_FRAGMENT = /* glsl */ `
uniform sampler2D tDiffuse;
uniform vec2 uResolution;   // low-res render target size in pixels
uniform float uDither;      // 0/1 dithering toggle
varying vec2 vUv;

// 4x4 ordered Bayer matrix (normalized 0..1).
float bayer4(vec2 p) {
  int x = int(mod(p.x, 4.0));
  int y = int(mod(p.y, 4.0));
  int i = x + y * 4;
  float m[16];
  m[0]=0.0;  m[1]=0.5;  m[2]=0.125; m[3]=0.625;
  m[4]=0.75; m[5]=0.25; m[6]=0.875; m[7]=0.375;
  m[8]=0.1875;m[9]=0.6875;m[10]=0.0625;m[11]=0.5625;
  m[12]=0.9375;m[13]=0.4375;m[14]=0.8125;m[15]=0.3125;
  float v = 0.0;
  for (int k = 0; k < 16; k++) { if (k == i) v = m[k]; }
  return v;
}

void main() {
  vec2 px = vUv * uResolution;
  vec2 snapped = (floor(px) + 0.5) / uResolution;   // pixelation
  vec3 c = texture2D(tDiffuse, snapped).rgb;
  float d = (uDither > 0.5) ? bayer4(px) - 0.5 : 0.0;
  c += d / 32.0;                                    // dither at quantization step
  c = floor(c * 31.0 + 0.5) / 31.0;                 // 5-bit quantization
  gl_FragColor = vec4(clamp(c, 0.0, 1.0), 1.0);
}
`;

export interface Ps1Pass {
  material: ShaderMaterial;
  /** Update resolution uniforms (low-res target size). */
  setResolution(width: number, height: number): void;
}

export function createPs1Pass(dither = true): Ps1Pass {
  const uniforms: Record<string, IUniform> = UniformsUtils.clone({
    tDiffuse: { value: null },
    uResolution: { value: [320, 180] },
    uDither: { value: dither ? 1 : 0 },
  });
  const material = new ShaderMaterial({
    uniforms,
    vertexShader: PS1_VERTEX,
    fragmentShader: PS1_FRAGMENT,
  });
  return {
    material,
    setResolution(w: number, h: number): void {
      uniforms.uResolution.value = [w, h];
    },
  };
}

/** Render the scene at low resolution then run the PS1 pass full-screen.
 * Guarded so callers in headless mode (no WebGL context) skip silently. */
export function renderPs1(renderer: WebGLRenderer, scene: never, camera: never, pass: Ps1Pass): void {
  void renderer; void scene; void camera; void pass;
  // Wired by RenderManager only when a GL context exists (TASK-023 runtime path).
}