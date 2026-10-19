// Prepares underground levels' textures off the main thread (a city's sewer sections, drawn
// as the view moves through them).
import type { Interior } from '../gen/protocol';
import { underField } from './underField';

export interface FieldRequest {
  id: number;
  it: Interior;
  level: number;
  clamp: boolean;
}

self.onmessage = (e: MessageEvent<FieldRequest>) => {
  const { id, it, level, clamp } = e.data;
  const f = underField(it, level, clamp);
  const moved = [f.data.buffer, f.cells.buffer, f.shapes.buffer, ...(f.cave ? [f.cave.buffer] : [])];
  (self as unknown as Worker).postMessage({ id, f }, moved);
};
