// Inspect both color chunks and decode pixels with a separate PNG implementation.
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {inflateSync} from 'node:zlib';
import {readdirSync,readFileSync} from 'node:fs';
import {join} from 'node:path';

const base=process.argv[2];
const expected={
  visualizer:[3840,2160], 'dsp-cover':[4000,4000], vertical:[2160,3840], panorama:[10800,2160],
};
const color={rec709:['Rec709-v4',[1,1,0,1]],srgb:['sRGB-v4',[1,13,0,1]],'display-p3':['DisplayP3-v4',[12,13,0,1]]};
const report={};
function png(file, size, encoding){
  const data=readFileSync(file);
  assert.deepEqual([data.readUInt32BE(16),data.readUInt32BE(20),data[24],data[25]],[...size,16,6]);
  let profile,cicp;
  for(let n=8;n<data.length;){
    const len=data.readUInt32BE(n), type=data.toString('ascii',n+4,n+8), value=data.subarray(n+8,n+8+len);
    if(type==='IDAT') break;
    if(type==='cICP') cicp=[...value];
    if(type==='iCCP') profile=inflateSync(value.subarray(value.indexOf(0)+2));
    n+=len+12;
  }
  const [icc,codes]=color[encoding];
  assert.deepEqual(cicp,codes);
  assert.deepEqual(profile,readFileSync(new URL('../../live-wheel/assets/color/'+icc+'.icc',import.meta.url)));
  const count=size[0]*size[1]*8;
  const result=spawnSync('ffmpeg',['-nostdin','-loglevel','error','-threads','1','-i',file,'-frames:v','1','-pix_fmt','rgba64le','-f','rawvideo','pipe:1'],{maxBuffer:count+65536});
  if(result.error || result.status!==0) throw Error(String(result.error || result.stderr));
  assert.equal(result.stdout.length,count);
  return result.stdout;
}
for(const dir of readdirSync(base,{withFileTypes:true}).filter(e=>e.isDirectory())){
  const path=join(base,dir.name);
  const m=JSON.parse(readFileSync(join(path,'plates.json')));
  assert.deepEqual(m.size,expected[m.layout]);
  const px=png(join(path,'projection.png'),m.size,m.encoding);
  const seen=Array.from({length:3},()=>new Uint8Array(65536));
  for(let i=0;i<px.length;i+=8){
    for(let c=0;c<3;c++) seen[c][px.readUInt16LE(i+c*2)]=1;
  }
  const distinctRGB=seen.map(a=>a.reduce((n,v)=>n+v,0));
  assert.ok(distinctRGB.every(n=>n>256));
  const guide=readFileSync(join(path,'guides.svg'),'utf8');
  assert.ok(guide.includes('projection.png'));
  report[m.layout]={size:m.size,encoding:m.encoding,cicp:m.cICP,distinctRGB};
  if(m.layout==='visualizer'){
    assert.equal(m.horizonY,1339);
    assert.ok(guide.includes('M0 1339H3840'));
  }else if(m.layout==='vertical'){
    assert.ok(guide.includes('y="576"') && guide.includes('height="499.2"') && guide.includes('y="3072"'));
  }else if(m.layout==='panorama'){
    const w=m.size[0],side=2160;
    for(const [n,name] of m.panels.entries()){
      const part=png(join(path,name),[side,side],m.encoding);
      for(let y=0;y<side;y++){
        const start=(w*y+side*n)*8;
        assert.ok(px.subarray(start,start+side*8).equals(part.subarray(y*side*8,(y+1)*side*8)));
      }
    }
    let wrap=0,neighbors=0;
    for(let y=0;y<side;y++) for(let c=0;c<3;c++){
      const p=x=>px.readUInt16LE((w*y+x)*8+2*c);
      wrap+=Math.abs(p(0)-p(w-1)); neighbors+=(Math.abs(p(0)-p(1))+Math.abs(p(w-1)-p(w-2)))/2;
    }
    report.panorama.identicalPanelPixels=w*side;
    report.panorama.meanEndToStartDifference=wrap/(side*3);
    report.panorama.meanAdjacentColumnDifference=neighbors/(side*3);
  }
}
assert.deepEqual(Object.keys(report).sort(),Object.keys(expected).sort());
console.log(JSON.stringify(report,null,2));
