// Bounded operator-invoked 4K integration proof. The actual encoder request,
// selection grants, native renderer, and source-audio decoder are shared with
// ordinary desktop exports. Commands bind paths from explicit process arguments.
export async function verifyWheelExport(selection, duration, status, projector) {
  const invoke = window.__TAURI__.core.invoke;
  const facts = {};
  try {
    const [selectionId,audioPath,outputPath,displays] = await invoke('select_wheel_export_fixture');
    facts.attachedDisplays=displays;
    await window.tssImportAudio([selectionId,audioPath]);
    const seconds=duration();
    if (!Number.isFinite(seconds) || seconds<=0) throw Error('Could not decode verification audio');
    if (!selection) throw Error('The verification wheel must be live');
    const el = id => document.getElementById(id);
    facts.audioDuration=seconds;
    facts.wheelOptions=el('mathmos-wheel').options.length-1;
    facts.wheel=el('mathmos-wheel').value;
    facts.gesture=el('wheel-gesture').value;
    facts.macroDeckReady=!el('macro-deck').hidden;
    facts.feedbackReady=!el('video-feedback').hidden;
    facts.projectorReady=!!projector && !el('projector').disabled;
    facts.routingApis={ enumerateDevices:typeof navigator.mediaDevices?.enumerateDevices==='function',
      selectAudioOutput:typeof navigator.mediaDevices?.selectAudioOutput==='function',
      cueSink:typeof AudioContext.prototype.setSinkId==='function',
      audienceSink:typeof HTMLMediaElement.prototype.setSinkId==='function' };
    const devices=await navigator.mediaDevices?.enumerateDevices?.() ?? [];
    facts.visibleAudioOutputs=devices.filter(d=>d.kind==='audiooutput' && d.deviceId && !['default','communications'].includes(d.deviceId)).length;
    if(facts.wheelOptions!==7 || !facts.macroDeckReady || !facts.feedbackReady || !facts.projectorReady) {
      throw Error('Editor controls failed verification');
    }
    const layerId = selection.layerId;
    const events = [];
    function stroke(slide,gesture,start,end,points) {
      for (let i=0; i<points.length; ++i) events.push({time:start+(end-start)*i/(points.length-1),
        layerId,slide,gesture,tugResponse:.4,phase:i?'move':'down',point:points[i]});
      events.push({time:end,layerId,slide,gesture,tugResponse:.4,phase:'up',point:points.at(-1)});
    }
    stroke('rear','tug',.25,.75,[[.41,.43],[.48,.48],[.56,.56],[.66,.65]]);
    stroke('front','rotate',1.10,1.60,[[.67,.50],[.64,.62],[.59,.73],[.5,.77]]);
    status('Exporting the two-cue 4K verification sequence…');
    await window.__TAURI__.event.listen('tss-wheel-export-progress', ({payload}) =>
      status(`4K verification · ${payload.completed} / ${payload.total} frames`));
    await invoke('export_native_wheel',{audioPath,outputPath,request:{
      layerId,wheel:selection.wheel,baseline:'neutral',duration:duration(),events}});
    el('audience-sink').value='mute';
    el('cue-sink').value='mute';
    el('projector').click();
    await projector.pending;
    facts.projectorOpened=projector.enabled && el('projector').getAttribute('aria-pressed')==='true';
    if (!facts.projectorOpened) throw Error('Projector open failed: '+el('projector-state').textContent);
    await new Promise(resolve=>setTimeout(resolve,1600));
    const projectorStats=await invoke('live_wheel_stats');
    facts.projectorPresents=projectorStats.projectorPresents;
    facts.projectorAgeMs=projectorStats.projectorAgeMs;
    if(!(projectorStats.projectorRunning && facts.projectorPresents>0)) throw Error('Projector did not present frames');
    facts.minimize=await invoke('verify_projector_visibility');
    const {hiddenMs,offscreenFrames,bothHiddenMs,bothHiddenFrames}=facts.minimize;
    if (!(hiddenMs>0 && offscreenFrames>0 && offscreenFrames<=Math.ceil(hiddenMs*60/1000)+1 &&
          bothHiddenMs>0 && bothHiddenFrames===0)) {
      throw Error('Unexpected presentation activity during minimize/restore verification');
    }
    el('projector').click();
    await projector.pending;
    facts.projectorClosed=!projector.enabled && el('projector').getAttribute('aria-pressed')==='false';
    if(!facts.projectorClosed) throw Error('Projector close failed: '+el('projector-state').textContent);
    await invoke('finish_wheel_export_verification',{error:null,facts:JSON.stringify(facts)});
  } catch(error) {
    status(String(error));
    await invoke('finish_wheel_export_verification',{error:String(error),facts:JSON.stringify(facts)});
  }
}
