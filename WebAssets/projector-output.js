// The cue and delayed program draw from the same Web Audio graph. Only the
// audience branch goes through the delay line and selected output endpoint.
export function audienceDelaySeconds(delayMs, trimMs) {
  return Math.max(0,Math.min(1.25,(delayMs+trimMs)/1000));
}

export function connectAudienceDelay(context,input,destination,seconds) {
  const delay=context.createDelay(1.25);
  delay.delayTime.value=seconds;
  input.connect(delay);
  delay.connect(destination);
  return delay;
}

export class AudienceAudio {
  constructor(context,input) {
    this.context=context;
    this.input=input;
    this.cue=context.createGain();
    this.cue.connect(context.destination);
    input.connect(this.cue);
    this.player=new Audio();
    this.player.autoplay=false;
    this.player.playsInline=true;
    this.delay=null;
    this.sink=null;
  }
  async route(cue,audience,delay) {
    const context=this.context;
    if (cue!=='mute' && (context.sinkId || '')!==cue) {
      if(!context.setSinkId) throw new Error('This runtime only exposes the system cue output.');
      await context.setSinkId(cue);
    }
    this.cue.gain.value=cue==='mute'?0:1;
    if(audience==='mute') {this.disconnect();return;}
    if((this.player.sinkId || '')!==audience) {
      if(!this.player.setSinkId) throw new Error('This runtime only exposes the system audience output.');
      await this.player.setSinkId(audience);
    }
    if (!this.sink) {
      this.sink=context.createMediaStreamDestination();
      this.player.srcObject=this.sink.stream;
    }
    this.seconds=delay;
    if(this.delay) this.delay.delayTime.value=delay;
    else this.flush();
    await this.player.play();
  }
  flush(){
    if(!this.sink) return;
    if(this.delay) {
      this.input.disconnect(this.delay);
      this.delay.disconnect();
    }
    this.delay=connectAudienceDelay(this.context,this.input,this.sink,this.seconds);
  }
  disconnect(){
    if(this.delay) {
      this.input.disconnect(this.delay);
      this.delay.disconnect();this.delay=null;
    }
    this.player.pause();
    this.player.srcObject=null;
    for(const track of this.sink?.stream.getTracks() || []) track.stop();
    this.sink=null;
  }
  async restore() {
    this.disconnect();
    try {
      if(this.context.setSinkId && this.context.sinkId) await this.context.setSinkId('');
    } finally { this.cue.gain.value=1; }
  }
}

export class ProjectorDeck {
  constructor({invoke,listen,prepare,ensureAudio,onChange}) {
    this.invoke=invoke;this.prepare=prepare;this.ensureAudio=ensureAudio;this.onChange=onChange;
    const el=id=>document.getElementById(id);
    this.toggle=el('projector');this.panel=el('projector-routing');
    this.delay=el('program-delay');this.trim=el('audio-trim');
    this.cue=el('cue-sink');this.audience=el('audience-sink');
    this.status=el('projector-state');this.enabled=false;this.pending=Promise.resolve();
    this.toggle.disabled=false;this.toggle.title='Open projector output';
    this.panel.hidden=false;
    this.toggle.addEventListener('click',()=>this.queue(()=>this.switch(!this.enabled)));
    for(const control of [this.delay,this.trim,this.cue,this.audience]){
      control.addEventListener('change',()=>this.queue(()=>this.reconfigure()));
    }
    const media=navigator.mediaDevices;
    const picker=el('audio-output-choose');picker.hidden=!media?.selectAudioOutput;
    picker.addEventListener('click',async()=>{
      try {
        const chosen=await media.selectAudioOutput();
        await this.refreshDevices(chosen);
        this.audience.value=chosen.deviceId;
        this.queue(()=>this.apply());
      } catch(error){this.status.textContent=String(error);}
    });
    media?.addEventListener('devicechange',()=>this.refreshDevices().catch(error=>{this.status.textContent=String(error);}));
    this.refreshDevices().catch(error=>{this.status.textContent=String(error);});
    listen('tss-projector-closed',()=>this.queue(()=>this.deactivate()))
      .catch(error=>{this.status.textContent=String(error);});
  }
  async refreshDevices(extra) {
    const outputs=(await navigator.mediaDevices?.enumerateDevices?.() ?? [])
      .filter(d=>d.kind==='audiooutput' && d.deviceId && !['default','communications'].includes(d.deviceId));
    if(extra && extra.deviceId!=='default' && !outputs.some(d=>d.deviceId===extra.deviceId)) outputs.push(extra);
    for(const control of [this.cue,this.audience]){
      const old=control.value;
      const options=[new Option('System default',''),new Option('Off','mute'),
        ...outputs.map((d,i)=>new Option(d.label || 'Audio output '+(i+1),d.deviceId))];
      control.replaceChildren(...options);
      control.value=options.some(o=>o.value===old)?old:'mute';
    }
    if(this.enabled) this.queue(()=>this.apply());
  }
  queue(task) {
    this.pending=this.pending.then(task).catch(error=>{this.status.textContent=String(error);});
    return this.pending;
  }
  async deactivate(){
    this.enabled=false;this.onChange();
    this.toggle.textContent='Projector ↗';this.toggle.setAttribute('aria-pressed','false');
    this.status.textContent='Projector closed.';
    if(this.audio) await this.audio.restore();
  }
  async reconfigure(){
    try { await this.apply(); }
    catch(error) {
      try { await this.invoke('set_projector_output',{enabled:false,delayMs:0}); }
      finally { await this.deactivate(); }
      throw error;
    }
  }
  async switch(enabled) {
    if(!enabled) {
      await this.invoke('set_projector_output',{enabled:false,delayMs:0});
      await this.deactivate();return;
    }
    await this.prepare();
    this.enabled=true;
    try {
      this.onChange();
      await this.apply();
    } catch(error) {
      try {await this.invoke('set_projector_output',{enabled:false,delayMs:0});}
      finally {await this.deactivate();}
      throw error;
    }
    this.toggle.textContent='Close projector';this.toggle.setAttribute('aria-pressed','true');
  }
  attach(context,input) { this.audio=new AudienceAudio(context,input); }
  async apply(){
    if(!this.enabled) return;
    const delayMs=Math.max(0,Math.min(1000,Number(this.delay.value)||0));
    const trimMs=Math.max(-250,Math.min(250,Number(this.trim.value)||0));
    const cue=this.cue.value, audience=this.audience.value;
    if(cue===audience && cue!=='mute') {
      // A single physical endpoint cannot monitor a direct cue and delayed
      // program independently. Let its chosen role be the audience output.
      this.cue.value='mute';
    }
    if(audience!=='mute' || this.cue.value!=='') this.ensureAudio();
    if(this.audio) await this.audio.route(this.cue.value,audience,audienceDelaySeconds(delayMs,trimMs));
    await this.invoke('set_projector_output',{enabled:true,delayMs});
    this.status.textContent=audience==='mute'
      ? `Projector open · ${delayMs} ms video delay · audience audio off`
      : `Projector open · ${delayMs} ms video delay · ${Math.round(audienceDelaySeconds(delayMs,trimMs)*1000)} ms audience audio delay`;
  }
  flush(){this.audio?.flush();}
}

