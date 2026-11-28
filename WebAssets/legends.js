import {equipmentFor} from './equipment.js';

const rigs = [
  {id:'joshua', artist:'Joshua Light Show', venue:'Fillmore East · New York',
    record:'The group combined film, carousel slides and liquid overheads behind one screen.',
    sources:[['Group archive','https://www.joshualightshow.com/about-classic/joshua-light-show-1967-68']],
    setup:'Liquid overhead with a film beam at 8% exposure. Offset the second image for a ghost.',
    rig:{a:'liquid-overhead', b:'film', temperatureA:3200,temperatureB:6500, levelB:.08, offsetX:.025, focusA:.5, focusB:.68}},
  {id:'ham', artist:'Bill Ham', venue:'Light Sound Dimension · San Francisco',
    record:'Ham stacked concave clock crystals on overhead projectors and combined their images on the screen.',
    sources:[['Artist interview','https://www.collectorsweekly.com/articles/lightman-fantastic/'],
      ['Stills','https://billhamlights.com/light-paintings-screenshots/'],['Videos','https://billhamlights.com/lightpaintings-videos/']],
    setup:'Two overlapping liquid overheads with independent focus and exposure. Raise one beam as the other fades.',
    rig:{a:'liquid-overhead', b:'liquid-overhead',temperatureA:3200,temperatureB:3200,levelB:.42,offsetX:0, focusA:.35,focusB:.7}}
];

export function installLegends({layers,preview,apply}) {
  const $=id=>document.getElementById(id), cards=$('legend-cards');
  let audition=null;
  function show() {
    const id=audition?.id;
    $('legend-status').textContent=id?'Auditioning '+rigs.find(r=>r.id===id).artist+'. Apply to save the rig.':'';
    $('legend-stop').hidden=!id;
    for (const button of cards.querySelectorAll('[data-audition]')) button.setAttribute('aria-pressed',String(button.dataset.audition===id));
  }
  function build({id,rig}) {
    const visible=layers().filter(l=>l.visible), routes=new Map();
    const beams=new Set(visible.map(l=>l.projector??'a'));
    if(visible.length>1 && beams.size===1){
      const beam=beams.has('a')?'b':'a';
      routes.set(beam==='b'?visible[0].id:visible.at(-1).id,beam);
    }
    return {id,routes,focus:rig.focusA,fields:{
      equipmentA:equipmentFor(rig.a), lampKelvin:rig.temperatureA, projectorAIntensity:1,
      projectorB:{equipment:equipmentFor(rig.b),enabled:true,lampKelvin:rig.temperatureB,focus:rig.focusB,
        intensity:rig.levelB,offsetX:rig.offsetX,offsetY:0}
    }};
  }
  function stop() { if(audition){audition=null;show();preview();} }
  for (const rig of rigs) {
    const card=document.createElement('article');card.className='legend-card';
    const name=document.createElement('h3');name.textContent=rig.artist;
    const venue=document.createElement('small');venue.textContent=rig.venue;
    const record=document.createElement('p');record.textContent=rig.record;
    const sources=document.createElement('div');sources.className='rig-actions';
    for(const [label,href] of rig.sources) {
      const link=document.createElement('a');link.href=href;link.target='_blank';link.rel='noreferrer';link.textContent=label+' ↗';sources.append(link);
    }
    const setup=document.createElement('p');
    const label=document.createElement('strong');label.textContent='TSS interpretation. ';
    setup.append(label,rig.setup);
    const illustration=document.createElement('div');illustration.className='rig-diagram';illustration.setAttribute('aria-hidden','true');
    for (const key of [rig.rig.a,rig.rig.b]) {
      const icon=document.createElement('i');icon.className='source-icon '+key;illustration.append(icon);
    }
    const auditions=document.createElement('button');auditions.className='button quiet';auditions.textContent='Audition';auditions.dataset.audition=rig.id;
    auditions.addEventListener('click',()=>{audition=build(rig);show();preview();});
    const applyButton=document.createElement('button');applyButton.className='button quiet';applyButton.textContent='Apply rig';applyButton.dataset.apply=rig.id;
    applyButton.addEventListener('click',()=>{const selected=build(rig);audition=null;show();apply(selected,rig.artist);});
    const controls=document.createElement('div');controls.className='rig-actions';controls.append(auditions,applyButton);
    card.append(illustration,name,venue,record,sources,setup,controls);cards.append(card);
  }
  $('legend-stop').addEventListener('click',stop);
  show();
  return {get audition(){return audition;},stop};
}
