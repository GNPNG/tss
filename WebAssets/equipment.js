// Profiles describe the fitted source. Exposure and lamp color are independently adjustable.
const museum='https://collection.sciencemuseumgroup.org.uk/objects/co205500/kodak-carousel-projector-model-550r';
export const equipmentProfiles = {
  custom: {name:'Custom', medium:'Configurable source', compatibility:'Liquid stages, images and moving footage.'},
  'liquid-overhead': {name:'Liquid overhead', medium:'Glass dishes', compatibility:'Load an oil layer for the liquid stage. Artwork overlays are a digital option.'},
  film: {name:'Film projector', medium:'Moving footage', compatibility:'Add artwork to load a moving clip. Live oil overlays are a digital option.'},
  'carousel-550r': {name:'Kodak Carousel 550R', medium:'35 mm slides', compatibility:'Add artwork to load a still transparency. Liquid and moving overlays are digital options.', source:museum}
};
const carousel='carousel-550r';
export function equipmentFor(profile) {
  return profile===carousel
    ? {profile,lens:{focalLengthMm:127,fNumber:3.5},lampMode:'high',lowIntensity:.6} : {profile};
}
export function validEquipment(eq) {
  if(eq===undefined) return true;
  if(!eq || !Object.hasOwn(equipmentProfiles,eq.profile)) return false;
  if(eq.profile!==carousel) return eq.lens===undefined && eq.lampMode===undefined && eq.lowIntensity===undefined;
  return eq.lens?.focalLengthMm===127 && eq.lens?.fNumber===3.5 && ['high','low'].includes(eq.lampMode) &&
    Number.isFinite(eq.lowIntensity) && eq.lowIntensity>=0 && eq.lowIntensity<=1;
}
export function lampScale(equipment) {
  return equipment?.lampMode==='low' ? equipment.lowIntensity : 1;
}
export function installEquipmentPicker(id, get, set) {
  const $=suffix=>document.getElementById('equip-'+id+'-'+suffix);
  const options=$('options'), inputs=new Map();
  for(const [key,{name,medium}] of Object.entries(equipmentProfiles)){
    const card=document.createElement('label');card.className='equipment-card';
    const input=document.createElement('input');input.type='radio';input.name='equipment-'+id;input.value=key;
    input.addEventListener('change',()=>{if(input.checked)set(equipmentFor(key));});
    const diagram=document.createElement('i');diagram.className='source-icon '+key;diagram.setAttribute('aria-hidden','true');
    const title=document.createElement('strong');title.textContent=name;
    const format=document.createElement('small');format.textContent=medium;
    card.append(input,diagram,title,format);options.append(card);inputs.set(key,input);
  }
  $('mode').addEventListener('change',event=>set({...get(),lampMode:event.target.value}));
  $('low').addEventListener('input',event=>set({...get(),lowIntensity:Number(event.target.value)/100}));
  return ()=>{
    const eq=get(), profile=eq?.profile??'custom';
    inputs.get(profile).checked=true;
    $('compatibility').textContent=equipmentProfiles[profile].compatibility;
    $('carousel').hidden=profile!==carousel;
    if(profile===carousel) {
      $('mode').value=eq.lampMode;
      $('low').value=Math.round(eq.lowIntensity*100);
      $('low-value').textContent=$('low').value+'%';
    }
  };
}
