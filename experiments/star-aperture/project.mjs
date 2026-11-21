import {createHash} from 'node:crypto';
import {copyFile, mkdir, readFile, writeFile} from 'node:fs/promises';
import {join} from 'node:path';
import {layerSettings, validateProject, emptyGestures, PROJECT_FORMAT} from '../../WebAssets/project-file.js';
import {newMedia} from '../../WebAssets/visual-media.js';
import {defaultBindings} from '../../WebAssets/timeline.js';
import {songGestureEvents} from '../../WebAssets/gesture-macros.js';

const [clips, dest] = process.argv.slice(2);
if (!clips || !dest) throw Error('Usage: node project.mjs prepared-clips-directory new-project-directory');
const {wheels} = JSON.parse(await readFile(new URL('../../WebAssets/assets/slide-presets.json', import.meta.url)));
const {swatches} = wheels.find(w=>w.id==='violet-blue');
const palette = [swatches.carrier, swatches.rear, swatches.overlap, swatches.front];
const audio = 'two-slide-cues.m4a';
const paths = [
  [audio, new URL('../slide-look-study/approval/windows-live/gesture-controls/export/'+audio, import.meta.url)],
  ['star-green.mp4', join(clips,'star-green.mp4')], ['moving-matte.mov', join(clips,'moving-matte.mov')],
];
const hashes = {};
for (const [name,path] of paths) hashes[name] = createHash('sha256').update(await readFile(path)).digest('hex');

function track(id,name) {
  return {id,name,visible:true,palette:'violet-blue',colors:palette,customColors:false,hue:0,speed:1,scale:1,opacity:1,
    nativeLive:true,motion:'wheel-flow',motionTake:0,visibleFilms:'both',slide:'rear',gesture:'tug',
    tugResponse:{rear:.4,front:.4},mousePressure:{rear:.5,front:.5},renderWidth:640};
}
function clip(id,name) {
  const source = newMedia({id,name,sha256:hashes[name],width:960,height:540,still:false,durationUs:3_000_000});
  return {...track(id,name), media:source};
}
function macro(id,name,songStart,slide,gesture,points) {
  const duration = .5;
  const input = points.map((point,i)=>({point, t:duration*i/(points.length-1),phase:i?'move':'down'}));
  input.push({point:points.at(-1),t:duration,phase:'up'});
  return {id,name,songStart,duration,enabled:true,fit:false,cue:'recording-position',muted:[],amount:1,step:0,cap:1,
    events:input.map(sample=>({...sample,layerId:1,slide,gesture,tugResponse:.4}))};
}
const oil=track(1,'Violet / Blue');
const mask=clip(2,'moving-matte.mov');
mask.visible=false;
const stars=clip(3,'star-green.mp4');
Object.assign(stars.media,{mode:'gradient',keyEnabled:true,maskA:2});
const gestures=emptyGestures();
gestures.macros=[
  macro('rear-tug','Pull the rear oil',.25,'rear','tug',[[.41,.43],[.48,.48],[.56,.56],[.66,.65]]),
  macro('front-turn','Turn the front slide',1.10,'front','rotate',[[.67,.50],[.64,.62],[.59,.73],[.5,.77]]),
];
const project={format:PROJECT_FORMAT,version:1,audio:{name:audio,sha256:hashes[audio]},position:0,
  layers:[stars,mask,oil].map(layerSettings),selected:1,nextId:4,focus:null,exportBaseline:'neutral',gestures,
  timeline:{duration:2,range:{start:0,end:2},bindings:defaultBindings()},
  feedback:{enabled:false,delay:200,strength:70,zoom:100,rotation:0}};
validateProject(project,new Set(wheels.map(w=>w.id)));
if (songGestureEvents(gestures.macros,[],1,2).length!==10) throw Error('Both optical gestures must survive portable playback.');
await mkdir(dest);
for(const [name,path] of paths) await copyFile(path,join(dest,name));
await writeFile(join(dest,'Star Aperture.tss'),JSON.stringify(project,null,2)+'\n',{flag:'wx'});
