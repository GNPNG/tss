export class NativeWheelPlates {
  constructor({button, invoke, listen, selection, busy, prepare, pause, status, refresh}) {
    Object.assign(this, {button, invoke, listen, selection, otherExportBusy: busy, prepare, pause, status, refreshExports: refresh});
    button.addEventListener('click', () => this.start().catch(error => status('Plate export failed: ' + String(error))));
  }

  refresh() {
    this.button.disabled = !!this.busy || this.otherExportBusy() || !this.selection();
  }

  async start() {
    if (this.busy || this.otherExportBusy()) return;
    if (!this.selection()) throw Error('Select a visible native wheel first.');
    this.busy = true;
    this.refreshExports();
    const label = this.button.textContent;
    this.button.textContent = 'Exporting 16-bit plates…';
    let unlisten;
    try {
      await this.prepare();
      const {layerId, wheel, motion, motionTake, visibleFilms, focus} = this.selection();
      let started = false;
      unlisten = await this.listen('tss-wheel-plate-progress', ({payload}) => {
        if (!started) { this.pause(); started = true; }
        this.status('4K still plates · ' + payload);
      });
      const folder = await this.invoke('export_native_wheel_plates', {
        selection: {layerId, wheel, motion, motionTake, visibleFilms, focus},
      });
      this.status(folder ? '4K wheel plates saved to ' + folder : 'Plate export cancelled');
      return folder;
    } finally {
      unlisten?.();
      this.busy = false;
      this.button.textContent = label;
      this.refreshExports();
    }
  }
}
