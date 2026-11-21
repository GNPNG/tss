// Independently decode every TIFF sample and every registered press PDF image.
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {readdirSync,readFileSync,mkdtempSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';

const [parent,profileDir]=process.argv.slice(2);
const report={};
const sha=data=>createHash('sha256').update(data).digest('hex');
function run(cmd,args,maxBuffer=8e6){
  const p=spawnSync(cmd,args,{maxBuffer,env:{...process.env,LC_ALL:'C',LANG:'C'}});
  if(p.status!==0)throw Error(cmd+': '+(p.error||p.stderr.toString().slice(-1500)));
  return p.stdout;
}
for(const entry of readdirSync(parent,{withFileTypes:true}).filter(e=>e.isDirectory())){
  const dir=join(parent,entry.name);
  const m=JSON.parse(readFileSync(join(dir,'mechanical.json')));
  const [w,h]=m.geometry.pixels;
  const max=w*h*8+65536, tif=join(dir,'artwork.tif'), pdf=join(dir,'press.pdf');
  const original=readFileSync(join(profileDir,m.destinationIcc));
  assert.deepEqual(run('exiftool',['-icc_profile','-b',tif]),original);
  const metadata=JSON.parse(run('exiftool',['-j','-ImageWidth','-ImageHeight','-BitsPerSample','-PhotometricInterpretation','-XResolution','-YResolution','-ResolutionUnit',tif]))[0];
  assert.deepEqual([metadata.ImageWidth,metadata.ImageHeight,metadata.BitsPerSample,metadata.PhotometricInterpretation,metadata.XResolution,metadata.YResolution,metadata.ResolutionUnit],[w,h,'16 16 16 16','CMYK',300,300,'inches']);
  const cmyk=run('magick',[tif,'-depth','16','-endian','MSB','cmyk:-'],max);
  assert.equal(cmyk.length,w*h*8);
  const distinct=Array.from({length:4},()=>new Uint8Array(65536));
  let peak=0;
  for(let i=0;i<cmyk.length;i+=8){
    let sum=0;
    for(let c=0;c<4;c++){
      const v=cmyk.readUInt16BE(i+2*c);sum+=v;distinct[c][v]=1;
    }
    peak=Math.max(peak,sum);
  }
  assert.equal(peak,m.inkCoverage.maxTotalCode);
  assert.ok(peak<=Math.floor(m.tacPercent*65535/100));
  for(let y=Math.floor(h/3);y<Math.floor(h/3)+24;y++)for(let x=Math.floor(w/5);x<Math.floor(w/5)+32;x++){
    assert.deepEqual([0,1,2,3].map(c=>cmyk.readUInt16BE((y*w+x)*8+2*c)),[39321,26214,26214,65535]);
  }
  const levels=distinct.map(a=>a.reduce((n,v)=>n+v,0));
  assert.ok(levels.every(v=>v>256));
  run('qpdf',['--check',pdf]);
  const boxes=run('pdfinfo',['-box',pdf]).toString();
  const box=kind=>boxes.match(new RegExp(kind+'Box:\\s+([^\\n]+)'))[1].trim().split(/\s+/).map(Number);
  const [cw,ch]=m.canvasMm.map(v=>v*72/25.4), b=m.geometry.bleedMm*72/25.4;
  for(const [kind,expected] of [['Media',[0,0,cw,ch]],['Bleed',[0,0,cw,ch]],['Trim',[b,b,cw-b,ch-b]]]){
    assert.ok(box(kind).every((v,i)=>Math.abs(v-expected[i])<.012),kind+' box in points');
  }
  const stream=id=>run('qpdf',['--show-object='+id,'--filtered-stream-data',pdf],max);
  assert.deepEqual(stream(6),original);
  assert.equal(sha(stream(5)),sha(cmyk));
  const stencil=stream(7);
  let marks=0;
  const stride=Math.ceil(w/8);
  for(let y=0;y<h;y++)for(let x=0;x<w;x++){
    if((stencil[y*stride+(x>>3)]&(128>>(x%8)))===0){
      assert.ok(x>=Math.floor(w*3/5)&&x<Math.floor(w*3/5)+32&&y>=Math.floor(h/3)&&y<Math.floor(h/3)+24);
      marks++;
    }
  }
  assert.equal(marks,768);
  const op=run('qpdf',['--show-object=8',pdf]).toString();
  assert.match(op,/\/OP true/);assert.match(op,/\/op true/);assert.match(op,/\/OPM 1/);
  const content=stream(4).toString('ascii');
  assert.match(content,/0 0 0 1 k/);assert.match(content,/\/Gold cs/);
  const folder=mkdtempSync(join(tmpdir(),'tss-press-rip-'));
  try {
    run('gs',['-q','-dBATCH','-dNOPAUSE','-dSAFER','-sDEVICE=tiffsep','-r72','-sOutputFile='+join(folder,'page.tif'),pdf]);
    const spot=readdirSync(folder).find(n=>n.includes(m.spotPlate));
    assert.ok(spot,'Ghostscript RIP must output named '+m.spotPlate+' plate');
    const statistics=run('magick',[join(folder,spot),'-format','%[fx:minima] %[fx:maxima]','info:']).toString().trim();
    assert.equal(statistics,'0 1');
  } finally {rmSync(folder,{recursive:true,force:true});}
  report[m.format.layout]={stock:m.format.stock,size:[w,h],coveragePixels:w*h,tacPercent:m.tacPercent,observedMaxTacPercent:peak*100/65535,distinctCMYK:levels,iccSha256:sha(original),processSha256:sha(cmyk),richBlackMarkPixels:768,registeredSpotPixels:marks,ripPlate:m.spotPlate};
}
assert.deepEqual(Object.keys(report).sort(),['label7','lp-gatefold']);
console.log(JSON.stringify(report,null,2));
