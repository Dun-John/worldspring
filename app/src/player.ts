// The players' window (play mode at a local table): opened by the DM's window, for the TV or a
// second screen. It builds its own map from the world file and follows the DM's play state.
import './render/glPrecision';
import { CanvasTextMetrics } from 'pixi.js';
import { mount } from 'svelte';
import Player from './play/Player.svelte';
import './app.css';
import { checkSupport } from './ui/support';

CanvasTextMetrics.experimentalLetterSpacing = true;

const target = document.getElementById('app')!;
if (checkSupport(target)) mount(Player, { target });
