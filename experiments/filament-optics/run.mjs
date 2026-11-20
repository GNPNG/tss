import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const viewer = process.argv[2] && resolve(process.argv[2]);
const backend = process.argv[3] || 'metal';
if (!viewer || !['metal','vulkan','opengl'].includes(backend)) {
  throw new Error('Usage: node run.mjs PATH_TO_GLTF_VIEWER [metal|vulkan|opengl]');
}
const here=dirname(fileURLToPath(import.meta.url));
const out=mkdtempSync(join(tmpdir(),'tss-filament-optics-'));
function run(executable, args, cwd, log) {
  const result=spawnSync(executable,args,{cwd,encoding:'utf8',maxBuffer:10_000_000});
  if (log) writeFileSync(log,(result.stdout||'')+(result.stderr||''));
  if (result.error || result.status!==0) throw new Error(`Command failed: ${basename(executable)} (${result.error||result.status}); see ${log||cwd}`);
}
run(process.execPath,[join(here,'generate.mjs'),out],out);
const sharp=join(out,'sharp.json'), focus=join(out,'focus.json');
writeFileSync(sharp,JSON.stringify([{name:'comparison',base:{'view.dof.enabled':false,'view.dithering':'NONE'},permute:{}}]));
writeFileSync(focus,JSON.stringify([{name:'focus',base:{'view.dof.enabled':true,'view.dithering':'NONE','viewer.cameraAperture':1},permute:{'viewer.cameraFocusDistance':[0.2,10]}}]));
for (const mode of ['default','uber']) for (const shape of ['clear','front','rear','overlap']) {
  if(mode==='uber'&&shape==='clear') continue;
  const dir=join(out,`${mode}-${shape}`);mkdirSync(dir);
  run(viewer,['-a',backend,...(mode==='uber'?['-u']:[]),'-b',sharp,'-e',join(out,`${shape}.gltf`)],dir,join(dir,'render.log'));
}
const focusDir=join(out,'focus-cases');mkdirSync(focusDir);
run(viewer,['-a',backend,'-b',focus,'-e',join(out,'overlap.gltf')],focusDir,join(focusDir,'render.log'));
function ppm(path) {
  const data=readFileSync(path), header=data.subarray(0,100).toString('ascii').match(/^P6\s+(\d+)\s+(\d+)\s+(\d+)\s/);
  if(!header || Number(header[3])!==255) throw new Error(`Unexpected PPM header: ${path}`);
  const width=Number(header[1]),height=Number(header[2]),offset=header[0].length;
  if(data.length-offset!==width*height*3) throw new Error(`Unexpected PPM dimensions: ${path}`);
  return {data,width,height,offset};
}
function pixels(mode, shape){return ppm(join(out,`${mode}-${shape}`,'comparison0.ppm'));}
const reference=pixels('default','clear'),w=reference.width,h=reference.height;
const roi={x0:Math.floor(w*270/614),x1:Math.floor(w*340/614),y0:Math.floor(h*280/640),y1:Math.floor(h*350/640)};
function compare(a,b) {
  if(a.width!==w||a.height!==h||b.width!==w||b.height!==h)throw new Error('Capture dimensions differ');
  const sumsA=[0,0,0],sumsB=[0,0,0];let count=0,identical=0,maxDifference=0;
  for(let y=roi.y0;y<roi.y1;y++)for(let x=roi.x0;x<roi.x1;x++) {
    let diff=0;const index=3*(y*w+x);
    for(let c=0;c<3;c++){const av=a.data[a.offset+index+c],bv=b.data[b.offset+index+c];sumsA[c]+=av;sumsB[c]+=bv;diff=Math.max(diff,Math.abs(av-bv));}
    identical+=diff===0;maxDifference=Math.max(maxDifference,diff);count++;
  }
  return {count,identical,maxDifference,meanA:sumsA.map(v=>v/count),meanB:sumsB.map(v=>v/count)};
}
const focusA=ppm(join(focusDir,'focus0.ppm')),focusB=ppm(join(focusDir,'focus1.ppm'));
if(focusA.width!==focusB.width||focusA.height!==focusB.height)throw new Error('Focus capture dimensions differ');
let total=0,changed=0;for(let k=0;k<focusA.width*focusA.height*3;k++){const d=Math.abs(focusA.data[focusA.offset+k]-focusB.data[focusB.offset+k]);total+=d;changed+=d>0;}
const reports={
  generatedAt:new Date().toISOString(), viewer, viewerSha256:createHash('sha256').update(readFileSync(viewer)).digest('hex'), backend,
  viewport:{width:w,height:h},roi,
  defaultFrontVsOverlap:compare(pixels('default','front'),pixels('default','overlap')),
  defaultClearVsRear:compare(reference,pixels('default','rear')),
  uberFrontVsOverlap:compare(pixels('uber','front'),pixels('uber','overlap')),
  focusComparison:{changedChannels:changed,meanAbsoluteDifference:total/(focusA.width*focusA.height*3)},
};
writeFileSync(join(out,'results.json'),JSON.stringify(reports,null,2)+'\n');
console.log(JSON.stringify({outputDirectory:out,...reports},null,2));
