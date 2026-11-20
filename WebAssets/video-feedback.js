// The native performance clock runs when no song is loaded. For a loaded song,
// only transport events publish new anchors, so slider and layout changes cannot seek it.
export class VideoFeedbackControls {
  constructor(root, sync) {
    this.root = root;
    this.sync = sync;
    this.epoch = 0;
    this.anchor = null;
    this.enabled = root.querySelector('[name=feedback-enabled]');
    this.params = root.querySelector('fieldset');
    this.inputs = Object.fromEntries([...this.params.querySelectorAll('input')]
      .map(input => [input.name, input]));
    this.enabled.addEventListener('change', () => {
      this.params.disabled = !this.enabled.checked;
      this.sync();
    });
    this.params.addEventListener('input', () => {
      this.readouts();
      this.sync();
    });
    this.readouts();
  }
  readouts() {
    for (const input of Object.values(this.inputs)) {
      const output = this.params.querySelector(`output[for="${input.id}"]`);
      output.textContent = input.value + input.dataset.unit;
    }
  }
  clock({ loaded, playing, position, ended }, discontinuity = false) {
    if (discontinuity) this.epoch++;
    this.anchor = loaded ? { epoch: this.epoch, playing, position, ended:!!ended, atUnixMs: Date.now() } : null;
    if (this.enabled.checked) this.sync();
  }
  config(projecting=false) {
    if (!this.enabled.checked) return { feedback: null, feedbackClock: projecting?this.anchor:null };
    const {delay, strength, zoom, rotation} = this.inputs;
    return {
      feedback: { delayMs: Number(delay.value), strength: Number(strength.value)/100,
        zoom: Number(zoom.value)/100, rotationDegrees: Number(rotation.value) },
      feedbackClock: this.anchor,
    };
  }
}
