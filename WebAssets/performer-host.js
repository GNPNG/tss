// Each simultaneous physical action owns one slide channel for its stroke.
// A device mapping can be changed while another device is performing.
export class PerformerRoutes {
  constructor({invoke, mapping, notice=()=>{}, now=()=>performance.now()}) {
    this.invoke=invoke;this.mapping=mapping;this.notice=notice;this.now=now;
    this.strokes=new Map();this.owners=new Map();
  }
  async batch(source, samples, arrived=this.now()) {
    const last=Math.max(...samples.map(s=>s.time));let run=null;
    const flush=async()=>{if(!run)return;const {stroke,points}=run;run=null;
      await this.invoke('perform_live_wheel_gesture',{batch:{layerId:stroke.layerId,slide:stroke.slide,
        gesture:stroke.mode,tugResponse:stroke.response,samples:points}});};
    for(const g of samples){
      const id=source+'/'+g.pointerId;
      let stroke=this.strokes.get(id);
      if(g.phase==='down'){
        if(stroke)continue;
        const m=this.mapping(source);if(!m?.layerId)continue;
        const mode=m.mode==='auto'?(g.pointerType==='mouse'?'pressure':'tug'):m.mode;
        const channel=m.layerId+'/'+m.slide+'/'+(mode==='pressure'?'pressure':'xy');
        if(this.owners.has(channel)){this.notice('Another performer is holding this slide control.');continue;}
        stroke={...m,mode,channel,source,last:[g.x,g.y],sent:false};this.strokes.set(id,stroke);this.owners.set(channel,id);
      }
      if(!stroke)continue;
      stroke.last=[g.x,g.y];const end=g.phase==='up'||g.phase==='cancel';
      const ageMs=Math.max(0,(last-g.time)*1000+this.now()-arrived);
      if(stroke.mode==='pressure'){
        await flush();
        if(g.pressure!==null&&g.pressure!==undefined && (g.pointerType!=='mouse'||g.pressureSource==='webkitForce') && (!end||stroke.sent)){
          await this.invoke('set_live_wheel_pressure',{layerId:stroke.layerId,slide:stroke.slide,center:stroke.last,
            phase:end?'up':stroke.sent?'move':'down',amount:end?0:g.pressure,ageMs});
          stroke.sent=true;
        }else if(end&&stroke.sent)await this.release(stroke);
      }else{
        if(run?.stroke!==stroke)await flush();
        run??={stroke,points:[]};run.points.push({phase:end?'up':g.phase,point:stroke.last,ageMs});stroke.sent=true;
      }
      if(end){this.owners.delete(stroke.channel);this.strokes.delete(id);}
    }
    await flush();
  }
  async release(s){
    if(!s.sent)return;
    if(s.mode==='pressure')return this.invoke('set_live_wheel_pressure',{layerId:s.layerId,slide:s.slide,amount:0,center:s.last,phase:'up',ageMs:0});
    return this.invoke('perform_live_wheel_gesture',{batch:{layerId:s.layerId,slide:s.slide,gesture:s.mode,tugResponse:s.response,samples:[{phase:'up',point:s.last,ageMs:0}]}});
  }
  async leave(source){
    for(const [id,s] of [...this.strokes])if(s.source===source || source===null){
      this.strokes.delete(id);this.owners.delete(s.channel);
      try{await this.release(s);}catch(e){this.notice(String(e));}
    }
  }
}

export function installPerformerDeck({invoke,listen,layers,selection,prepare}){
  const panel=document.getElementById('performer-deck');panel.hidden=false;
  const find=id=>panel.querySelector('#'+id), msg=t=>find('performer-status').textContent=t;
  const toggle=find('performer-toggle'), url=find('performer-url');const devices=new Map();
  const keys={cert:null,key:null};
  let listening=false,closing=false,busy=Promise.resolve(),backlog=0;
  const routes=new PerformerRoutes({invoke,mapping:source=>devices.get(source)?.mapping(),notice:msg});
  const opts=(el,items,previous)=>{el.replaceChildren(...items.map(([v,text])=>new Option(text,v)));if(items.some(([v])=>String(v)===String(previous)))el.value=previous;};
  function device(source,name) {
    const row=document.createElement('fieldset'),legend=document.createElement('legend');legend.textContent=name;row.append(legend);
    const select=(label)=>{const l=document.createElement('label'),e=document.createElement('select');l.append(label,e);row.append(l);return e;};
    const layer=select('Layer'),slide=select('Slide'),action=select('Control');
    const mappedLayer = () => layers().find(l=>String(l.id)===layer.value);
    function refreshSlide() {
      const single = mappedLayer()?.filmDepth != null;
      slide.parentElement.hidden = slide.disabled = single;
      if (single) slide.value = 'rear';
    }
    function refresh() {
      const old=layer.value||String(selection()?.layerId||'');
      opts(layer,layers().map(l=>[l.id,l.name]),old);
      refreshSlide();
    }
    opts(slide,[['rear','Rear'],['front','Front']],selection()?.slide||'rear');
    refresh();layer.addEventListener('focus',refresh);layer.addEventListener('change',refreshSlide);
    opts(action,[['auto','Pencil swirl / trackpad pressure'],['tug','Swirl material'],['pressure','Compress'],['rotate','Rotate glass']],'auto');
    find('performer-devices').append(row);
    devices.set(source,{row,mapping:()=>{
      const l=mappedLayer();
      if (!l) return null;
      const target=l.filmDepth != null ? 'rear' : slide.value;
      return {layerId:l.id,slide:target,mode:action.value,response:l.tugResponse?.[target]??0.4};
    }});
  }
  async function clear(){
    await routes.leave(null);for(const v of devices.values())v.row.remove();devices.clear();
    listening=false;find('performer-pairing').hidden=true;url.value='';toggle.textContent='Start listener';find('performer-config').disabled=false;
  }
  async function stop(reason){
    if(closing)return;closing=true;
    try{await invoke('stop_performer');await clear();msg(reason);}finally{closing=false;}
  }
  const registration=listen('tss-performer-batch',({payload:e})=>{
    const arrived=performance.now();++backlog;
    busy=busy.then(async()=>{
      if(closing)return;if(backlog>256){await stop('Input backlog exceeded. Listener stopped.');return;}
      if(e.kind==='joined'&&!devices.has(e.source)){device(e.source,e.name);msg('Connected: '+e.name);}
      if(e.kind==='batch'){await routes.batch(e.source,e.samples,arrived);}
      if(e.kind==='left'){await routes.leave(e.source);devices.get(e.source)?.row.remove();devices.delete(e.source);}
      if(e.kind==='stopped'){await clear();msg('Performance listener closed.');}
    }).catch(async err=>{await routes.leave(e.source??null);msg(String(err));}).finally(()=>--backlog);
  });
  for(const kind of ['cert','key'])find('performer-'+kind).addEventListener('click',async()=>{
    try{const path=await invoke('choose_performer_tls',{key:kind==='key'});if(path){keys[kind]=path;find('performer-'+kind+'-name').textContent=path.split(/[\\/]/).at(-1);}}catch(e){msg(String(e));}
  });
  toggle.addEventListener('click',async()=>{
    toggle.disabled=true;
    try{
      if(listening){await stop('Listener stopped.');return;}
      if(!keys.cert||!keys.key)throw Error('Choose a trusted certificate chain and its PEM private key.');
      await prepare();await registration;
      const pair=await invoke('start_performer',{config:{bind:find('performer-bind').value.trim(),host:find('performer-host').value.trim(),port:Number(find('performer-port').value),...keys}});
      listening=true;url.value=pair.url+'/#'+pair.secret;find('performer-pairing').hidden=false;
      toggle.textContent='Stop listener';find('performer-config').disabled=true;msg('Waiting for performers.');
    }catch(e){msg(String(e));}finally{toggle.disabled=false;}
  });
  find('performer-copy').addEventListener('click',async()=>{try{await navigator.clipboard.writeText(url.value);msg('Pairing link copied.');}catch{url.type='text';url.select();msg('Copy the selected pairing link.');}});
  return {drain:()=>busy, clear};
}
