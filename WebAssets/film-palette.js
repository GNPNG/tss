import {LIQUID_PALETTES} from './liquid-renderer.js';

export function rotateColor(hex, degrees) {
  const values = hex.slice(1).match(/../g).map((part) => parseInt(part, 16) / 255);
  const max = Math.max(...values), min = Math.min(...values), delta = max - min;
  let hue = 0;
  if (delta) {
    if (max === values[0]) hue = ((values[1] - values[2]) / delta) % 6;
    else if (max === values[1]) hue = (values[2] - values[0]) / delta + 2;
    else hue = (values[0] - values[1]) / delta + 4;
  }
  hue = ((hue * 60 + degrees) % 360 + 360) % 360;
  const chroma = delta, x = chroma * (1 - Math.abs((hue / 60) % 2 - 1)), m = min;
  let rgb;
  if (hue < 60) rgb = [chroma, x, 0];
  else if (hue < 120) rgb = [x, chroma, 0];
  else if (hue < 180) rgb = [0, chroma, x];
  else if (hue < 240) rgb = [0, x, chroma];
  else if (hue < 300) rgb = [x, 0, chroma];
  else rgb = [chroma, 0, x];
  return rgb.map((component) => component + m);
}

export function isNativePalette(layer, slidePresets) {
  return !!layer && (layer.nativeLive || layer.simulationSeed != null) && !layer.media &&
    (Object.hasOwn(LIQUID_PALETTES, layer.palette) || slidePresets.has(layer.palette));
}

// A single cell transmits white through its carrier and one dye through its oil.
// Archived paired fields retain their original carrier, rear, overlap and front optics.
export function paletteFields(layer) {
  if (!layer || layer.media) return {};
  if (layer.filmDepth == null && !Object.hasOwn(LIQUID_PALETTES, layer.palette) && !layer.customColors && !(layer.hue % 360)) return {};
  const swatches = layer.colors.map(color => {
    if (!layer.hue) return parseInt(color.slice(1), 16);
    const [r, g, b] = rotateColor(color, layer.hue).map(v => Math.round(v * 255));
    return (r << 16) | (g << 8) | b;
  });
  if (layer.filmDepth != null) return {swatches:[0xffffff, swatches[0], swatches[0], 0xffffff]};
  return {swatches};
}
