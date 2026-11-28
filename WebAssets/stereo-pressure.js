// Rear-to-front cells from the pressure study. Each cell has its own
// solver and one pigmented film, addressed through that cell's rear channel.
const cells = [
  {depth:0, seed:83724, dye:0x7234cf},
  {depth:.5, seed:92843, dye:0x18d7df},
  {depth:1, seed:17312, dye:0xff728c},
];

function pressureRig(usedIds) {
  let next = Number.MAX_SAFE_INTEGER;
  const stack = cells.map(({depth, seed, dye}) => {
    while (usedIds.has(next)) next--;
    return {layerId:next--, wheel:'orchid', motion:'wheel-flow', opacity:1,
      swatches:[0xffffff, dye, dye, 0xffffff], visibleFilms:'rear', simulation:{depth, seed, rate:1}};
  });
  const {layerId, wheel, motion, swatches, visibleFilms, simulation} = stack.at(-1);
  return {layerId, wheel, motion, swatches, visibleFilms, simulation, stack,
    slide:'rear', gesture:'pressure', hardwarePressureOnly:true, mousePressure:0,
    renderWidth:640, focus:.72, keepPresenting:true};
}

export class StereoPressureDeck {
  constructor({button, output, invoke, listen, usedIds, prepare, restore, change, status, blocked}) {
    Object.assign(this, {button, output, invoke, usedIds, prepare, restore, change, status, blocked});
    this.selection = null;
    this.available = false;
    this.busy = false;
    this.display = '';
    this.lastPresent = null;
    this.registration = listen('tss-stereo-closed', ({payload}) => this.disconnect(payload));
    button.addEventListener('click', () => this.toggle());
    this.refresh();
  }

  refresh() {
    this.button.disabled = this.busy || this.restoring || !this.available || (!this.selection && this.blocked());
    this.button.textContent = this.selection ? 'Stop live 3D' : 'Live 3D pressure';
    this.button.setAttribute('aria-pressed', String(!!this.selection));
    this.button.title = this.selection ? 'Return to the project preview' :
      this.available ? 'Project three live oil layers in side-by-side 3D on an external display' :
        'Live 3D pressure requires the bundled fluid solver';
  }

  async initialize(available) {
    this.available = available;
    this.refresh();
    if (await this.invoke('stereo_startup_requested')) await this.toggle();
  }

  async toggle() {
    if (this.busy || this.restoring) return;
    if (!this.selection && (!this.available || this.blocked())) {
      this.status(this.available ? 'Finish the current export before starting live 3D.' : this.button.title);
      return;
    }
    this.busy = true;
    this.refresh();
    try {
      await this.registration;
      if (this.selection) {
        await this.invoke('set_stereo_output', {enabled:false});
        await this.clear();
        this.status('Live 3D stopped. Project preview restored.');
      } else {
        const rig = pressureRig(this.usedIds());
        this.selection = rig;
        this.change();
        try {
          await this.prepare();
          const name = await this.invoke('set_stereo_output', {enabled:true});
          if (this.selection !== rig) return;
          this.display = name;
          this.output.textContent = name + ' · waiting for frame timing and trackpad force';
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
    const layers = this.selection?.stack;
    if (!layers) return;
    this.restoring = true;
    this.selection = null;
    this.display = '';
    this.lastPresent = null;
    this.output.textContent = '';
    try {
      this.change();
      await this.restore(layers.map(l => l.layerId));
    } finally {
      this.restoring = false;
      this.refresh();
    }
  }

  async disconnect(reason) {
    if (!this.selection) return;
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
    if (!this.display || !stats.projectorRunning || stats.layerId !== this.selection?.layerId) return;
    const now = performance.now();
    const count = stats.projectorPresents;
    const last = this.lastPresent;
    const cadence = last && count >= last.count && now > last.time ?
      ((count - last.count) * 1000 / (now - last.time)).toFixed(1) + ' FPS' : 'measuring';
    this.lastPresent = {count, time:now};
    const percent = force => Number.isFinite(force) ? Math.round(force * 100) + '%' : '—';
    this.output.textContent = this.display + ' · TV ' + cadence + ' · ' + count + ' stereo frames' +
      ' · trackpad ' + percent(stats.trackpadPressure) + ' · peak ' + percent(stats.trackpadPressurePeak) +
      ' · ' + (stats.trackpadPressureEvents ?? 0) + ' pressure events';
  }
}
