// Shader programs compiled in the background (KHR_parallel_shader_compile), so a slow driver
// compile doesn't hold the main thread: the same sources linked here and left to the driver,
// polled between frames; Pixi's own compile of them afterwards comes from the driver's cache.
import type { GlProgram, Renderer, WebGLRenderer } from 'pixi.js';

/** Resolves once the driver has compiled `programs` (at once without the extension). */
export async function precompile(renderer: Renderer, programs: GlProgram[]): Promise<void> {
  const gl = (renderer as WebGLRenderer).gl as WebGL2RenderingContext | undefined;
  const ext = gl?.getExtension('KHR_parallel_shader_compile') as { COMPLETION_STATUS_KHR: number } | null | undefined;
  if (!gl || !ext) return;
  const made = programs.map((p) => {
    const prog = gl.createProgram()!;
    for (const [type, src] of [
      [gl.VERTEX_SHADER, p.vertex],
      [gl.FRAGMENT_SHADER, p.fragment],
    ] as const) {
      const s = gl.createShader(type)!;
      gl.shaderSource(s, src ?? '');
      gl.compileShader(s);
      gl.attachShader(prog, s);
      gl.deleteShader(s);
    }
    gl.linkProgram(prog);
    return prog;
  });
  while (made.some((p) => !gl.getProgramParameter(p, ext.COMPLETION_STATUS_KHR))) await new Promise((r) => setTimeout(r, 50));
  for (const p of made) gl.deleteProgram(p);
}
