// A frame-indexed analysis shared by playback and offline rendering.
// The three bands use a pair of simple low-pass filters, then RMS envelopes.
export async function analyzeAudio(buffer, onProgress = () => {}, isCurrent = () => true) {
  const analysisFps = 30;
  const samplesPerFrame = buffer.sampleRate / analysisFps;
  const frameCount = Math.ceil(buffer.duration * analysisFps);
  const channels = Array.from({ length: buffer.numberOfChannels }, (_, channel) => buffer.getChannelData(channel));
  const lows = new Float64Array(channels.length);
  const highs = new Float64Array(channels.length);
  const lowAlpha = 1 - Math.exp(-2 * Math.PI * 250 / buffer.sampleRate);
  const highAlpha = 1 - Math.exp(-2 * Math.PI * 2000 / buffer.sampleRate);
  const frames = [];
  let previous = { bass: 0, mid: 0, treble: 0, energy: 0 };

  for (let frame = 0; frame < frameCount; frame++) {
    if (!isCurrent()) return null;
    const begin = Math.floor(frame * samplesPerFrame);
    const end = Math.min(buffer.length, Math.floor((frame + 1) * samplesPerFrame));
    let bass = 0, mid = 0, treble = 0, energy = 0;
    for (let i = begin; i < end; i++) {
      for (let channel = 0; channel < channels.length; channel++) {
        const sample = channels[channel][i];
        lows[channel] += lowAlpha * (sample - lows[channel]);
        highs[channel] += highAlpha * (sample - highs[channel]);
        bass += lows[channel] ** 2;
        mid += (highs[channel] - lows[channel]) ** 2;
        treble += (sample - highs[channel]) ** 2;
        energy += sample ** 2;
      }
    }
    const count = Math.max(1, (end - begin) * channels.length);
    const envelope = (sum, old) => old * 0.6 + Math.min(1, Math.sqrt(sum / count) * 4) * 0.4;
    previous = {
      bass: envelope(bass, previous.bass), mid: envelope(mid, previous.mid),
      treble: envelope(treble, previous.treble), energy: envelope(energy, previous.energy),
    };
    frames.push(previous);
    if (frame % 150 === 149) {
      onProgress((frame + 1) / frameCount);
      await new Promise((resolve) => setTimeout(resolve, 0));
    }
  }
  return { fps: analysisFps, frames };
}

export function featuresAt(analysis, time) {
  if (!analysis?.frames.length) return { energy: 0, bass: 0, mid: 0, treble: 0 };
  const index = Math.max(0, Math.min(analysis.frames.length - 1, Math.floor(time * analysis.fps)));
  return analysis.frames[index];
}
