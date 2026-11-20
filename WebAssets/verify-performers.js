// Runs only with --verify-wheel-export and --verify-performers. All network
// connections in this proof are TLS loopback connections on an ephemeral port.
export async function verifyPerformers(deck,layerId){
  const {invoke}=window.__TAURI__.core,{listen}=window.__TAURI__.event;
  if(!await invoke('start_performer_verification'))return null;
  const received=[],steps=new Map();let failure=null,recording=false;
  const events=await listen('tss-performer-batch',({payload:e})=>{
    if(e.kind==='proof-step')steps.set(e.name,true);
    if(e.kind==='proof-error')failure=e.error;
  });
  const capture=await listen('tss-live-gesture',({payload})=>received.push(payload));
  async function until(test,description){
    const end=performance.now()+6000;
    while(!test()){if(failure)throw Error(failure);if(performance.now()>end)throw Error('Timed out waiting for '+description);await new Promise(r=>setTimeout(r,15));}
  }
  async function stage(op,ack){await invoke('performer_verification_step',{step:op});await until(()=>steps.has(ack),ack);await deck.drain();}
  const channel=(slide,gesture)=>received.filter(e=>e.layerId===layerId&&e.slide===slide&&e.gesture===gesture).flatMap(e=>e.samples);
  try{
    await stage('join','joined');
    const rows=[...document.querySelectorAll('#performer-devices fieldset')];
    const pen=rows.find(r=>r.querySelector('legend').textContent==='Proof Pencil');
    const pad=rows.find(r=>r.querySelector('legend').textContent==='Proof Trackpad');
    if(!pen||!pad)throw Error('Missing independently paired controller');
    for(const [row,slide] of [[pen,'front'],[pad,'rear']]){const [layers,slides]=row.querySelectorAll('select');layers.value=String(layerId);slides.value=slide;}
    await invoke('set_live_wheel_capture_enabled',{enabled:true});recording=true;
    await stage('hold','held');
    await until(()=>channel('rear','pressure').some(s=>Math.abs(s.amount-.75)<.001)&&channel('front','tug').some(s=>s.phase==='move'),'native pressure and tug capture');
    await stage('release','released');
    await until(()=>channel('rear','pressure').some(s=>s.phase==='up'&&s.amount===0)&&channel('front','tug').some(s=>s.phase==='up'),'both native releases');
    const sequence=a=>a.map(s=>({phase:s.phase,point:s.point,amount:s.amount??null}));
    return {transport:'paired loopback HTTPS',sources:2,layerId,rearPressure:sequence(channel('rear','pressure')),frontPencil:sequence(channel('front','tug'))};
  }finally{
    if(recording)await invoke('set_live_wheel_capture_enabled',{enabled:false});
    await invoke('stop_performer');await deck.clear();events();capture();
  }
}
