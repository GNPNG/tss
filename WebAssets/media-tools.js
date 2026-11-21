export function installMediaTools(root, invoke) {
  if (!root || !invoke) return;
  const get = id => root.querySelector('#'+id);
  const state = get('media-tools-status'), versions = get('media-tools-versions');
  const manager = get('media-tools-manager'), install = get('media-tools-install'), check = get('media-tools-check');
  let report = null, busy = false;
  root.hidden = false;

  function render(result) {
    report = result;
    versions.replaceChildren(...result.tools.map(tool => {
      const item = document.createElement('li');
      item.textContent = [tool.name, tool.version, tool.path].filter(Boolean).join(' · ') +
        (tool.issues.length ? ' — '+tool.issues.join(' ') : '');
      return item;
    }));
    manager.textContent = [result.installCommand, !result.managerAvailable && !result.ready ? result.managerHelp : ''].filter(Boolean).join('\n');
    install.hidden = result.ready || !result.installCommand;
  }

  async function perform(installing) {
    if (busy) return;
    busy = true;
    install.disabled = check.disabled = true;
    state.textContent = installing ? 'Installing FFmpeg and FFprobe…' : 'Checking local FFmpeg and FFprobe…';
    try {
      const result = await invoke(installing ? 'install_media_tools' : 'check_media_tools');
      render(result);
      state.textContent = result.ready ? 'FFmpeg media tools are ready.' : 'FFmpeg media tools need attention.';
    } catch (error) {
      // A failed install may still have changed one executable. Show the new
      // local capability report alongside the package manager's original error.
      if (installing) {
        try { render(await invoke('check_media_tools')); }
        catch { report = null; versions.replaceChildren(); manager.textContent = ''; }
      } else {
        report = null; versions.replaceChildren(); manager.textContent = '';
      }
      state.textContent = (installing ? 'Installation failed. ' : 'Check failed. ') + String(error);
    } finally {
      busy = false;
      check.disabled = false;
      install.disabled = !report?.managerAvailable || !!report?.ready;
      if (!report) install.hidden = true;
    }
  }
  root.addEventListener('toggle', () => { if (root.open && !report) perform(false); });
  check.addEventListener('click', () => perform(false));
  install.addEventListener('click', () => perform(true));
}
