// Bounded operator-invoked 4K integration proof. The actual encoder request,
// selection grants, native renderer, and source-audio decoder are shared with
// ordinary desktop exports. Commands bind paths from explicit process arguments.
import { wheelMovieRequest } from './native-wheel-export.js';
export async function verifyWheelExport(selection, duration, status, projector,performerDeck) {
  const invoke = window.__TAURI__.core.invoke;
  const facts = {};
  try {
    const [selectionId,audioPath,outputPath,displays,pressureExport] = await invoke('select_wheel_export_fixture');
    facts.attachedDisplays=displays;
    await window.tssImportAudio([selectionId,audioPath]);
    const seconds=duration();
    if (!Number.isFinite(seconds) || seconds<=0) throw Error('Could not decode verification audio');
    if (!selection) throw Error('The verification wheel must be live');
    const el = id => document.getElementById(id);
    if (selection.stack?.some(l=>l.media)) facts.artworkTracks=selection.stack.filter(l=>l.media).map(l=>({
      id:l.layerId, matteOnly:!!l.matteOnly, mode:l.media.mode, keyEnabled:l.media.keyEnabled,
      maskA:l.media.maskA,maskB:l.media.maskB,durationUs:l.media.durationUs,still:l.media.still,
    }));
    facts.audioDuration=seconds;
    facts.wheelOptions=el('mathmos-wheel').options.length-1;
    facts.wheel=el('mathmos-wheel').value;
    facts.motion=el('wheel-motion').value;
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
    if (pressureExport) {
      const point=[.48,.52];
      for(const [time,phase,amount] of [[1.10,'down',0],[1.15,'move',.2],[1.20,'move',.48],[1.30,'move',.8],
        [1.50,'move',.8],[1.55,'move',.48],[1.60,'move',.2],[1.65,'up',0]]) {
        events.push({time,layerId,slide:'front',gesture:'pressure',phase,point,amount});
      }
      facts.frontPressurePeak=.8;
    }
    stroke('front','rotate',1.10,1.60,[[.67,.50],[.64,.62],[.59,.73],[.5,.77]]);
    events.sort((a,b)=>a.time-b.time);
    const {verifyPerformers}=await import('./verify-performers.js');
    const performerProof=await verifyPerformers(performerDeck,layerId);
    if(performerProof)facts.performers=performerProof;
    status('Exporting the two-cue 4K verification sequence…');
    await window.__TAURI__.event.listen('tss-wheel-export-progress', ({payload}) =>
      status(`4K verification · ${payload.completed} / ${payload.total} frames`));
    await invoke('export_native_wheel',{audioPath,outputPath,
      request:wheelMovieRequest(selection, duration(), events, 'neutral')});
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
