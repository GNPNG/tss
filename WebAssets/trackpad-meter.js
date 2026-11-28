const sensitivityKey = 'tss.trackpad.sensitivity';
const depthKey = 'tss.stereo.depth';
const widthKey = 'tss.stereo.renderWidth';

function readSetting(key, minimum, maximum, fallback) {
  try {
    const text = localStorage.getItem(key);
    if (text != null && text.trim() !== '') {
      const saved = Number(text);
      if (saved >= minimum && saved <= maximum) return saved;
    }
  } catch { /* An unavailable store still permits this performance. */ }
  return fallback;
}

function saveSetting(key, value) {
  try { localStorage.setItem(key, String(value)); }
  catch { /* Keep the selected value for this session. */ }
}

export class TrackpadMeter {
  constructor(root, change, listen) {
    this.root = root;
    this.target = null;
    this.forceInputs = root.querySelector('.force-inputs');
    this.depthControls = root.querySelector('#stereo-depth-controls');
    this.name = root.querySelector('#trackpad-target');
    this.force = root.querySelector('#trackpad-force');
    this.load = root.querySelector('#trackpad-load');
    this.forceValue = root.querySelector('#trackpad-force-value');
    this.loadValue = root.querySelector('#trackpad-load-value');
    const slider = root.querySelector('#trackpad-sensitivity');
    const value = root.querySelector('#trackpad-sensitivity-value');
    const showSensitivity = () => {
      const label = Number(this.sensitivity.toFixed(2)) + '×';
      value.textContent = label;
      slider.setAttribute('aria-valuetext', label);
    };
    this.sensitivity = readSetting(sensitivityKey, .25, 4, 1);
    slider.value = Math.log2(this.sensitivity);
    showSensitivity();
    slider.addEventListener('input', () => {
      this.sensitivity = 2 ** slider.valueAsNumber;
      showSensitivity();
      change();
    });
    slider.addEventListener('change', () => saveSetting(sensitivityKey, this.sensitivity));
    const depthSlider = root.querySelector('#stereo-depth');
    const depthValue = root.querySelector('#stereo-depth-value');
    const showDepth = () => {
      const label = depthSlider.value + '%';
      depthValue.textContent = label;
      depthSlider.setAttribute('aria-valuetext', label);
    };
    this.stereoDepth = readSetting(depthKey, 0, 1, .2);
    this.stereoWidth = readSetting(widthKey, 640, 960, 960) === 640 ? 640 : 960;
    depthSlider.value = Math.round(this.stereoDepth * 100);
    showDepth();
    depthSlider.addEventListener('input', () => {
      this.stereoDepth = depthSlider.valueAsNumber / 100;
      showDepth();
      change();
    });
    depthSlider.addEventListener('change', () => saveSetting(depthKey, this.stereoDepth));
    this.registration = listen('tss-trackpad-pressure', ({payload}) => {
      if (this.target?.layerId !== payload.layerId || this.target?.slide !== payload.slide) return;
      this.display(payload.raw, payload.applied);
    });
  }

  setTarget(target) {
    if (target?.layerId !== this.target?.layerId || target?.slide !== this.target?.slide) this.display(0, 0);
    this.target = target;
    this.forceInputs.hidden = !target;
    this.root.hidden = !target && this.depthControls.hidden;
    this.name.textContent = target?.name ?? '';
  }

  setStereoActive(active) {
    this.depthControls.hidden = !active;
    this.root.hidden = !active && !this.target;
  }

  setStereoWidth(width) {
    if (width === 640 || width === 960) {
      this.stereoWidth = width;
      saveSetting(widthKey, width);
    }
  }

  display(raw, applied) {
    if (!Number.isFinite(raw) || !Number.isFinite(applied)) return;
    this.force.value = raw * 100;
    this.load.value = applied * 100;
    this.forceValue.textContent = Math.round(this.force.value) + '%';
    this.loadValue.textContent = Math.round(this.load.value) + '%';
  }
}
