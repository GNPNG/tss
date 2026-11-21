export function hasRelease(motion) {
  return motion === 'coral-release' || motion === 'wheel-with-coral-beads';
}

// The editor forwards viewport/selection changes only. The native renderer
// captures mouse motion and presents directly into its child surface.
export class NativeWheelSurface {
  constructor(invoke, element, selection, report) {
    this.invoke = invoke;
    this.element = element;
    this.selection = selection;
    this.report = report;
    this.pending = Promise.resolve();
    this.desiredKey = 'null';
    this.surfaceRequested = false;
    this.pollInFlight = false;
    const layoutChanged = () => {
      if (!this.queuedLayout) this.queuedLayout = requestAnimationFrame(() => {
        this.queuedLayout = 0;
        this.sync();
      });
    };
    new ResizeObserver(layoutChanged).observe(element);
    window.addEventListener('resize', layoutChanged);
    window.addEventListener('scroll', layoutChanged, true);
    document.addEventListener('visibilitychange', () => this.sync());
    setInterval(() => this.poll(), 750);
  }

  config() {
    const selected = this.selection();
    if (!selected) return null;
    const covered = document.hidden || !!document.querySelector('dialog[open]:not(#native-wheel-export-progress)');
    const retain = selected.keepPresenting || this.exporting;
    if (covered && !retain) return null;
    const b = this.element.getBoundingClientRect();
    if (b.width <= 1 || b.height <= 1) return null;
    const outside = b.right <= 0 || b.bottom <= 0 || b.left >= innerWidth || b.top >= innerHeight;
    if(!retain && outside) return null;
    // Rounding both edges keeps the native child flush with the webview
    // content at fractional display scales or webview zoom.
    const px = (value) => Math.round(value * devicePixelRatio);
    const x = px(b.left), y = px(b.top);
    return { ...selected, previewHidden:covered || outside, rect: { x, y, width: px(b.right) - x, height: px(b.bottom) - y } };
  }

  sync() {
    const config = this.config();
    const key = JSON.stringify(config);
    if (key === this.desiredKey) return this.pending;
    this.desiredKey = key;
    if (config) this.surfaceRequested = true;
    // Keep the newest desired state while serializing native view changes. A slow
    // show cannot arrive after a subsequent hide for a modal or layer switch.
    this.pending = this.pending.then(async () => {
      if (key !== this.desiredKey) return;
      try {
        await this.invoke('set_live_wheel', { config });
        this.surfaceRequested = !!config;
        if (config) this.poll();
      } catch (error) {
        if (key === this.desiredKey) this.report({ error: String(error) });
      }
    });
    return this.pending;
  }

  async poll() {
    if (this.pollInFlight || !this.surfaceRequested || !this.selection() || this.desiredKey === 'null') return;
    const key = this.desiredKey;
    this.pollInFlight = true;
    try {
      const stats = await this.invoke('live_wheel_stats');
      if (key === this.desiredKey) this.report(stats);
    } catch (error) {
      if (key === this.desiredKey) this.report({ error: String(error) });
    } finally { this.pollInFlight = false; }
  }
}
