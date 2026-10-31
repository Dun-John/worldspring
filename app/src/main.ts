import './render/glPrecision';
import { CanvasTextMetrics } from 'pixi.js';
import { mount } from 'svelte';
import App from './App.svelte';
import './app.css';
import { checkSupport } from './ui/support';

// Letter-spaced labels (continents, seas, ranges, towns) drawn with the canvas's own letter
// spacing where the browser has it, in one pass, rather than one character at a time (which
// cost up to ~40 ms a label on this laptop, mid-flight).
CanvasTextMetrics.experimentalLetterSpacing = true;

const target = document.getElementById('app')!;
if (checkSupport(target)) {
  // `?gallery=1`: every sprite the map draws, labelled (src/dev/gallery.ts).
  if (new URLSearchParams(location.search).has('gallery')) void import('./dev/gallery').then((g) => g.runGallery(target));
  else mount(App, { target });
}
