// Imported first by each entry, before anything calls `GlProgram.from`: Pixi's default of
// mediump is a real 16-bit float on mobile GPUs (Adreno: range 2^15, 10 bits), which breaks the
// shaders' world coordinates and noise; desktop GPUs run it as highp, so only phones ever showed it.
import { GlProgram } from 'pixi.js';

GlProgram.defaultOptions.preferredFragmentPrecision = 'highp';
