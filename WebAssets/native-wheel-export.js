import { detailScaleFields, materialMotionFields } from './material-motion.js';
import { hasRelease } from './native-wheel.js';

// Capture the selected track and its scheduled performance once. Frames stay native.
export function wheelMovieRequest(selection, duration, events, baseline) {
  return {
    layerId: selection.layerId,
    wheel: selection.wheel,
    ...(selection.swatches ? {swatches:selection.swatches} : {}),
    ...(selection.focus != null ? {focus:selection.focus} : {}),
    ...(selection.equipmentA ? {equipmentA:structuredClone(selection.equipmentA)} : {}),
    ...(selection.lampKelvin != null ? {lampKelvin:selection.lampKelvin} : {}),
    ...(selection.projectorAIntensity != null ? {projectorAIntensity:selection.projectorAIntensity} : {}),
    ...(selection.projectorB ? {projectorB:{...selection.projectorB}} : {}),
    ...materialMotionFields(selection),
    ...detailScaleFields(selection),
    ...(selection.visibleFilms ? {visibleFilms: selection.visibleFilms} : {}),
    ...(selection.motion ? {motion: selection.motion, motionTake: selection.motionTake} : {}),
    ...(selection.stack ? {stack: selection.stack} : {}),
    duration,
    events,
    baseline: baseline === 'auto'
      ? (events.length || hasRelease(selection.motion) || selection.stack?.some(l => hasRelease(l.motion)) ? 'neutral' : 'current')
      : baseline,
  };
}

export class NativeWheelExport {
  constructor({invoke, listen, selection, source, getEvents, finishRecording, pause, prepare, sync, status}) {
    Object.assign(this, {invoke, listen, selection, source, getEvents, finishRecording, pause, prepare, sync, status});
    const byId = id => document.getElementById(id);
    this.group = byId('native-wheel-export');
    this.button = byId('export-native-wheel');
    this.pose = byId('native-export-baseline');
    this.dialog = byId('native-wheel-export-progress');
    this.progress = byId('native-export-progress-bar');
    this.count = byId('native-export-count');
    this.cancelButton = byId('cancel-native-export');
    this.button.addEventListener('click', () => this.start().catch(error => this.status(
      String(error) === 'Export cancelled.' ? 'Slide export cancelled' : 'Slide export failed: ' + String(error))));
    const cancel = () => {
      this.cancelButton.disabled = true;
      this.count.textContent = 'Cancelling after the current frame…';
      this.invoke('cancel_native_wheel_export').catch(error => {
        this.cancelButton.disabled = false;
        this.count.textContent = String(error);
      });
    };
    this.dialog.addEventListener('cancel', e => { e.preventDefault(); cancel(); });
    this.cancelButton.addEventListener('click', cancel);
  }

  refresh() {
    const {audioPath, duration, proceduralBusy} = this.source();
    const selection = this.selection();
    this.group.hidden = !selection;
    this.button.textContent = selection?.stack?.some(l => l.media) ? 'Export artwork program in 4K' : selection?.stack?.length > 1 ? 'Export slide stack in 4K' : 'Export selected slide in 4K';
    this.button.disabled = this.busy || proceduralBusy || !audioPath || !(duration > 0);
  }

  async start() {
    if (this.busy) return;
    const {audioPath, duration, proceduralBusy} = this.source();
    const selection = this.selection();
    if (!selection || !audioPath || !duration || proceduralBusy) return;
    this.busy = true;
    this.refresh();
    let unlisten;
    try {
      const outputPath = await this.invoke('choose_export_path');
      if (!outputPath) return;
      const current = this.selection();
      if (current?.layerId !== selection.layerId || current.wheel !== selection.wheel ||
          JSON.stringify(current.swatches) !== JSON.stringify(selection.swatches) || current.motion !== selection.motion || current.motionTake !== selection.motionTake ||
          current.materialMotion?.rear !== selection.materialMotion?.rear || current.materialMotion?.front !== selection.materialMotion?.front ||
          JSON.stringify(current.equipmentA)!==JSON.stringify(selection.equipmentA) ||
          current.focus !== selection.focus || current.lampKelvin !== selection.lampKelvin ||
          current.projectorAIntensity !== selection.projectorAIntensity || JSON.stringify(current.projectorB)!==JSON.stringify(selection.projectorB) ||
          current.visibleFilms !== selection.visibleFilms || current.detailScale !== selection.detailScale || JSON.stringify(current.stack) !== JSON.stringify(selection.stack) ||
          this.source().audioPath !== audioPath || this.source().proceduralBusy) throw Error('The selected slide, films, motion or song changed.');
      await this.finishRecording();
      this.pause();
      await this.prepare();
      const ids = selection.stack?.filter(layer => !layer.media).map(layer => layer.layerId) ?? [selection.layerId];
      const events = ids.flatMap(id => this.getEvents(id, duration)).sort((a, b) => a.time - b.time);
      const request = wheelMovieRequest(selection, duration, events, this.pose.value);
      this.count.textContent = 'Preparing 3840 × 2160 optical render…';
      this.progress.removeAttribute('value');
      this.cancelButton.disabled = false;
      unlisten = await this.listen('tss-wheel-export-progress', ({payload}) => {
        this.progress.max = payload.total;
        this.progress.value = payload.completed;
        if (!this.cancelButton.disabled) this.count.textContent = payload.completed === payload.total
          ? 'Finishing audio and video…' : 'Rendered ' + payload.completed + ' of ' + payload.total + ' frames';
      });
      this.dialog.showModal();
      await this.invoke('export_native_wheel', {audioPath, outputPath, request});
      this.status('Program saved to ' + outputPath);
    } finally {
      if (this.dialog.open) this.dialog.close();
      unlisten?.();
      this.busy = false;
      this.refresh();
      await this.sync();
    }
  }
}
