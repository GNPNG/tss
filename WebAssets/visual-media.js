import {materialMotionFields} from './material-motion.js';
import { bindProjectMedia as bindVisualMedia } from './project-media.js';
export { bindVisualMedia };
export function newMedia(grant) {
  const media={sha256:grant.sha256,name:grant.name,offsetUs:0,rate:1,loop:true,mode:'color',
    keyEnabled:false,keyColor:'#00ff00',keyThreshold:0.12,keySoftness:0.08,gradientA:'#ff2070',gradientB:'#08cfff',gradientRate:0.25,
    bounce:0,blur:0,distortion:0,maskA:null,maskB:null,maskOp:'intersect',shape:'none',tunnelSpeed:1,tunnelBass:0};
  bindVisualMedia({media},grant);
  return media;
}
const rgb=color=>[1,3,5].map(i=>parseInt(color.slice(i,i+2),16)/255);
export function programLayers(layers, isWheel) {
  const available=layers.filter(l=>l.media?l.media.assetId!=null:isWheel(l));
  const references=new Set(available.filter(l=>l.visible&&l.media).flatMap(l=>[l.media.maskA,l.media.maskB]).filter(x=>x!=null));
  const availableIds=new Set(available.map(l=>l.id));
  const result=[];
  for(const layer of [...available].reverse()){
    if (!layer.visible && !references.has(layer.id)) continue;
    const common={layerId:layer.id,opacity:layer.opacity,...(!layer.visible?{matteOnly:true}:{}),
      ...(layer.envelope!=null?{envelope:layer.envelope}:{})};
    if(!layer.media){
      result.push({...common,wheel:layer.palette,motion:layer.motion,motionTake:layer.motionTake,...materialMotionFields(layer),visibleFilms:layer.visibleFilms??'both'});
      continue;
    }
    const {sha256,name,width,height,...m}=layer.media;
    // A missing linked matte closes its aperture until the operator relinks it.
    if (common.matteOnly) {m.maskA=null; m.maskB=null;}
    if([m.maskA,m.maskB].some(id=>id!=null&&!availableIds.has(id))) common.opacity=0;
    result.push({...common,media:{...m,keyColor:rgb(m.keyColor),gradientA:rgb(m.gradientA),gradientB:rgb(m.gradientB),
      maskA:availableIds.has(m.maskA)?m.maskA:null,maskB:availableIds.has(m.maskB)?m.maskB:null}});
  }
  return result;
}
export function installVisualMedia({invoke,selected,layers,add,changed,report,isWheel}) {
  const $=id=>document.getElementById(id);
  const pane=$('artwork-settings');
  const fields=Array.from(pane.querySelectorAll('[data-media]'));
  let importing=false;
  function refresh(){
    const layer=selected(),m=layer?.media;
    pane.hidden=!m;
    if(!m)return;
    $('artwork-source').textContent=m.assetId==null?m.name+' · offline':m.name;
    $('artwork-relink').hidden=m.assetId!=null;
    for(const el of fields){
      const k=el.dataset.media;
      if(k==='maskA'||k==='maskB'){
        el.replaceChildren(new Option('None',''));
        for(const l of layers()) if(l.id!==layer.id && (l.media || isWheel(l))) el.add(new Option(l.name,String(l.id)));
      }
      if (k==='tunnelSpeed'||k==='tunnelBass') {el.closest('label').hidden=m.shape!=='star-tunnel'; el.value=m[k]??(k==='tunnelSpeed'?1:0);continue;}
      if(el.type==='checkbox')el.checked=m[k];
      else el.value=m[k]==null?'':String(k==='offsetUs'?m[k]/1e6:m[k]);
    }
  }
  for(const el of fields) el.addEventListener('change',()=>{
    const m=selected()?.media;if(!m)return;
    const k=el.dataset.media;
    if(k==='maskA'||k==='maskB')m[k]=el.value?Number(el.value):null;
    else if(el.type==='checkbox')m[k]=el.checked;
    else if(el.type==='number'||el.type==='range'){
      if(!el.checkValidity()||!Number.isFinite(el.valueAsNumber)){refresh();return;}
      m[k]=k==='offsetUs'?Math.round(1e6*el.valueAsNumber):el.valueAsNumber;
    }else m[k]=el.value;
    changed('Edit artwork '+k);
  });
  $('artwork-add').addEventListener('click',async()=>{
    if(importing)return;importing=true;$('artwork-add').disabled=true;
    try{const grant=await invoke('choose_visual_media');if(grant)add(newMedia(grant));}catch(error){report('Media import failed: '+error);}
    finally{importing=false;$('artwork-add').disabled=false;}
  });
  $('artwork-relink').addEventListener('click',async()=>{
    const layer=selected();if(!layer?.media)return;
    try{const grant=await invoke('relink_visual_media',{sha256:layer.media.sha256});if(grant){bindVisualMedia(layer,grant);changed('Relink artwork');}}
    catch(error){report('Media relink failed: '+error);}
  });
  return {refresh};
}
