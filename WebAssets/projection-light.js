import {equipmentProfiles,installEquipmentPicker,lampScale} from './equipment.js';

// One lamp illuminates each projector's slides. Projector B is optional.
const daylight = 6500;
const newBeam = () => ({enabled:true, lampKelvin:daylight, focus:null, intensity:1, offsetX:0, offsetY:0});
const lampNames = new Map([[3200,'Tungsten'],[6500,'Neutral']]);

export function effectiveLighting(fields) {
  const a=lampScale(fields.equipmentA), b=fields.projectorB;
  return {...fields,...(a!==1?{projectorAIntensity:(fields.projectorAIntensity??1)*a}:{}),
    ...(b?{projectorB:{...b,intensity:b.intensity*lampScale(b.equipment)}}:{})};
}

export function installProjectionLight({changed, selected}) {
  let lampKelvin, projectorAIntensity, equipmentA, projectorB;
  const el = id => document.getElementById(id);
  const lamp = id => id==='a' ? lampKelvin ?? daylight : projectorB?.lampKelvin ?? daylight;
  const equipment=id=>id==='a'?equipmentA:projectorB?.equipment;
  const pickers=['a','b'].map(id=>installEquipmentPicker(id,()=>equipment(id),value=>{
    if(id==='a')equipmentA=value;
    else if(projectorB)projectorB.equipment=value;
    edited();
  }));
  function editLamp(id, value) {
    if (id==='a') lampKelvin=value;
    else projectorB.lampKelvin=value;
  }
  function refresh() {
    for (const picker of pickers) picker();
    for (const id of ['a','b']) {
      const kelvin=lamp(id);
      el('lamp-'+id+'-kelvin').value=kelvin;
      el('lamp-'+id+'-preset').value=lampNames.has(kelvin)?String(kelvin):'custom';
    }
    const aIntensity=Math.round((projectorAIntensity ?? 1)*100);
    el('lamp-a-intensity').value=aIntensity;
    el('lamp-a-intensity-value').textContent=aIntensity+'%';
    const layer=selected();
    el('layer-projector').disabled=!layer;
    el('layer-projector').value=layer?.projector ?? 'a';
    el('lamp-b-enabled').checked=!!projectorB?.enabled;
    el('lamp-b-settings').disabled=!projectorB;
    const b=projectorB ?? newBeam();
    for(const key of ['focus','intensity','offsetX','offsetY']) {
      const input=el('lamp-b-'+key), value=Math.round((b[key]??.5)*100);
      input.value=value;
      el(input.id+'-value').textContent=b[key]===null ? 'Slide setting' : value+'%';
    }
    el('lamp-b-reset-focus').disabled=b.focus===null;
  }
  function edited(layout=false) { refresh(); changed(layout); }
  for (const id of ['a','b']) {
    const preset=el('lamp-'+id+'-preset'), input=el('lamp-'+id+'-kelvin');
    preset.addEventListener('change',()=>{
      if (preset.value==='custom') { input.focus();input.select();return; }
      editLamp(id,Number(preset.value));edited();
    });
    input.addEventListener('change',()=>{
      if (input.value!=='' && input.checkValidity()) editLamp(id,Number(input.value));
      edited();
    });
  }
  el('lamp-a-intensity').addEventListener('input',event=>{
    projectorAIntensity=Number(event.target.value)/100;
    edited();
  });
  el('lamp-b-ghost').addEventListener('click',()=>{
    if (!projectorB) return;
    projectorB.intensity=.08;
    edited();
    el('lamp-b-intensity').dispatchEvent(new Event('change',{bubbles:true}));
  });
  el('lamp-b-enabled').addEventListener('change',event=>{
    const first=!projectorB;
    projectorB??=newBeam();
    projectorB.enabled=event.target.checked;
    edited(first);
  });
  for(const key of ['focus','intensity','offsetX','offsetY']){
    el('lamp-b-'+key).addEventListener('input',event=>{
      if (!projectorB) return;
      projectorB[key]=Number(event.target.value)/100;
      edited();
    });
  }
  el('lamp-b-reset-focus').addEventListener('click',()=>{
    if (!projectorB) return;
    projectorB.focus=null;
    edited();
    el('lamp-b-focus').dispatchEvent(new Event('change',{bubbles:true}));
  });
  el('layer-projector').addEventListener('change', event=>{
    const layer=selected();
    if (!layer) return;
    if(event.target.value==='b') {
      projectorB??=newBeam();
      layer.projector='b';
    } else delete layer.projector;
    edited(true);
  });
  return {
    refresh,
    restore(doc) {
      lampKelvin=doc.lampKelvin;projectorAIntensity=doc.projectorAIntensity;
      equipmentA=doc.equipmentA&&structuredClone(doc.equipmentA);
      projectorB=doc.projectorB&&structuredClone(doc.projectorB);
      refresh();
    },
    fields() {
      const result={};
      if(lampKelvin!=null)result.lampKelvin=lampKelvin;
      if(projectorAIntensity!=null)result.projectorAIntensity=projectorAIntensity;
      if(equipmentA)result.equipmentA=structuredClone(equipmentA);
      if(projectorB)result.projectorB=structuredClone(projectorB);
      return result;
    },
    get grouped(){return !!projectorB;},
    get usesLiveOptics(){return lamp('a')!==daylight || (projectorAIntensity??1)!==1 || equipmentA?.profile==='carousel-550r' || !!projectorB?.enabled;},
    groupLabel(id){
      const k=lamp(id), name=lampNames.get(k), profile=equipment(id)?.profile;
      const source=profile?equipmentProfiles[profile].name+' · ':'';
      return 'Projector '+id.toUpperCase()+' · '+source+(name?name+' · ':'')+k+' K'+(id==='b'&&!projectorB?.enabled?' · Off':'');
    }
  };
}
