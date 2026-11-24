// Shader programs compiled in the background (KHR_parallel_shader_compile), so a slow driver
// compile doesn't hold the main thread: the sources are linked here and left to the driver,
// polled between frames, and the finished programs are handed to Pixi as its own, so it never
// links them again. (Relying on the driver's cache for Pixi's second link did not hold: on
// Direct3D the battlemap's ground shader was compiled again in full, on the main thread.)
import { GlProgramData, defaultValue, extractAttributesFromGlProgram, getUboData, getUniformData } from 'pixi.js';
import type { GlProgram, Renderer, WebGLRenderer } from 'pixi.js';

type Hash = Record<string, GlProgramData | undefined>;

/** Resolves once the driver has compiled `programs` (at once without the extension). */
export async function precompile(renderer: Renderer, programs: GlProgram[]): Promise<void> {
  const gl = (renderer as WebGLRenderer).gl as WebGL2RenderingContext | undefined;
  const ext = gl?.getExtension('KHR_parallel_shader_compile') as { COMPLETION_STATUS_KHR: number } | null | undefined;
  if (!gl || !ext) return;
  const hash = () => (renderer as WebGLRenderer).shader as unknown as { _programDataHash: Hash; _gl: WebGL2RenderingContext };
  // (Not those Pixi has already, nor one with transform feedback: Pixi's own link sets that up.)
  const todo = [...new Set(programs)].filter((p) => !hash()._programDataHash[p._key] && !p.transformFeedbackVaryings);
  const made = todo.map((p) => {
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
  while (!gl.isContextLost() && made.some((p) => !gl.getProgramParameter(p, ext.COMPLETION_STATUS_KHR))) await new Promise((r) => setTimeout(r, 50));
  // (A lost context took the programs with it; a restored one has a new hash and warms again.)
  if (gl.isContextLost() || hash()._gl !== gl) return;
  made.forEach((prog, i) => {
    const p = todo[i];
    // (Linked meanwhile by Pixi, or failed: Pixi's own link then reports the error as usual.)
    if (hash()._programDataHash[p._key] || !gl.getProgramParameter(prog, gl.LINK_STATUS)) {
      gl.deleteProgram(prog);
      return;
    }
    // As Pixi's `generateProgram` after its link.
    p._attributeData = extractAttributesFromGlProgram(prog, gl, !/^[ \t]*#[ \t]*version[ \t]+300[ \t]+es[ \t]*$/m.test(p.vertex ?? ''));
    p._uniformData = getUniformData(prog, gl);
    p._uniformBlockData = getUboData(prog, gl);
    const uniforms: Record<string, { location: WebGLUniformLocation | null; value: unknown }> = {};
    for (const name in p._uniformData) {
      const d = p._uniformData[name];
      uniforms[name] = { location: gl.getUniformLocation(prog, name), value: defaultValue(d.type, d.size) };
    }
    hash()._programDataHash[p._key] = new GlProgramData(prog, uniforms as ConstructorParameters<typeof GlProgramData>[1]);
  });
}
