// Capture the selected track and its scheduled performance once. Frames stay native.
export function wheelMovieRequest(selection, duration, events, baseline) {
  return {
    layerId: selection.layerId,
    wheel: selection.wheel,
    duration,
    events,
    baseline: baseline === 'auto' ? (events.length ? 'neutral' : 'current') : baseline,
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
      String(error) === 'Export cancelled.' ? 'Wheel export cancelled' : 'Wheel export failed: ' + String(error))));
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
    this.group.hidden = !this.selection();
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
      if (this.selection()?.layerId !== selection.layerId || this.selection()?.wheel !== selection.wheel ||
          this.source().audioPath !== audioPath || this.source().proceduralBusy) throw Error('The selected wheel or song changed.');
      await this.finishRecording();
      this.pause();
      await this.prepare();
      const request = wheelMovieRequest(selection, duration, this.getEvents(selection.layerId, duration), this.pose.value);
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
      this.status('Selected wheel saved to ' + outputPath);
    } finally {
      if (this.dialog.open) this.dialog.close();
      unlisten?.();
      this.busy = false;
      this.refresh();
      await this.sync();
    }
  }
}
