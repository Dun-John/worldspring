// Imported first by each entry, before anything calls `GlProgram.from`: Pixi's default of
// mediump is a real 16-bit float on mobile GPUs (Adreno: range 2^15, 10 bits), which breaks the
// shaders' world coordinates and noise; desktop GPUs run it as highp, so only phones ever showed it.
// Samplers are separate: a fragment shader's default is `lowp sampler2D`, whose lookups Adreno
// returns as 16-bit floats, so every sampler is declared `uniform highp sampler2D` (heights read
// lowp drew 5-ft contours over flat ground and a stair-stepped shore on an S24). Not a
// `precision highp sampler2D;` line: Pixi adds its float precision only when the source doesn't
// already start with `precision`.
import { GlProgram } from 'pixi.js';

GlProgram.defaultOptions.preferredFragmentPrecision = 'highp';
