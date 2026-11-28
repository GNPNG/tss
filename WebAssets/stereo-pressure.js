export class StereoPressureDeck {
  constructor({button, output, invoke, listen, prepare, sync, change, status, unavailableReason}) {
    Object.assign(this, {button, output, invoke, prepare, sync, change, status, unavailableReason});
    this.active = false;
    this.targetId = null;
    this.available = false;
    this.busy = false;
    this.display = '';
    this.lastPresent = null;
    this.registration = listen('tss-stereo-closed', ({payload}) => this.disconnect(payload));
    button.addEventListener('click', () => this.toggle());
    this.refresh();
  }

  refresh() {
    const reason = this.available
      ? (!this.active && this.unavailableReason()) : 'Live 3D pressure requires the bundled fluid solver';
    this.button.disabled = this.busy || this.restoring || !!reason;
    this.button.textContent = this.active ? 'Stop live 3D' : 'Live 3D pressure';
    this.button.setAttribute('aria-pressed', String(this.active));
    this.button.title = reason || (this.active ? 'Stop side-by-side output' :
      'Show the current project in side-by-side 3D on the external display');
  }

  async initialize(available) {
    this.available = available;
    this.refresh();
    if (await this.invoke('stereo_startup_requested')) await this.toggle();
  }

  async toggle() {
    if (this.busy || this.restoring) return;
    if (!this.active && (!this.available || this.unavailableReason())) {
      this.status(this.button.title);
      return;
    }
    this.busy = true;
    this.refresh();
    try {
      await this.registration;
      if (this.active) {
        await this.invoke('set_stereo_output', {enabled:false});
        await this.clear();
        this.status('Live 3D stopped. Project preview restored.');
      } else {
        this.active = true;
        this.change();
        try {
          await this.prepare();
          const name = await this.invoke('set_stereo_output', {enabled:true});
          if (!this.active) return;
          this.display = name;
          this.output.textContent = name + ' · measuring frame rate';
          this.status('Live 3D on ' + name + '. Select side-by-side 3D on the TV.');
        } catch (error) {
          await this.clear();
          throw error;
        }
      }
    } catch (error) {
      this.status('Live 3D: ' + (error.message || error));
    } finally {
      this.busy = false;
      this.refresh();
    }
  }

  async clear() {
    if (!this.active) return;
    this.restoring = true;
    this.active = false;
    this.targetId = null;
    this.display = '';
    this.lastPresent = null;
    this.output.textContent = '';
    try {
      this.change();
      await this.sync();
    } finally {
      this.restoring = false;
      this.refresh();
    }
  }

  async disconnect(reason) {
    if (!this.active) return;
    try {
      await this.clear();
      this.status('Live 3D closed. ' + (reason || 'Project preview restored.'));
    } catch (error) {
      this.status('Live 3D: ' + (error.message || error));
    } finally {
      this.refresh();
    }
  }

  report(stats) {
    if (!this.active || !this.display || !stats.projectorRunning) return;
    const now = performance.now();
    const count = stats.projectorPresents;
    const last = this.lastPresent;
    const cadence = last && count >= last.count && now > last.time ?
      ((count - last.count) * 1000 / (now - last.time)).toFixed(1) + ' FPS' : 'measuring';
    this.lastPresent = {count, time:now};
    this.output.textContent = this.display + ' · TV ' + cadence + ' · ' + count + ' stereo frames' +
      ' · peak ' + Math.round(100 * (stats.trackpadPressurePeak ?? 0)) + '% · ' +
      (stats.trackpadPressureEvents ?? 0) + ' pressure events';
  }
}
