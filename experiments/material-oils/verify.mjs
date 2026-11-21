// Verify raw drawn geometry, physical sample times, solver bounds, and cache payloads.
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {readFileSync, writeFileSync} from 'node:fs';
import {join} from 'node:path';
import {gunzipSync} from 'node:zlib';

const [run, measure, assets, output] = process.argv.slice(2);
if (!output) throw Error('Usage: node verify.mjs RUN_DIR MEASURE_EXE ASSETS_DIR OUTPUT_DIR');
const sha = data => createHash('sha256').update(data).digest('hex');
const file = (folder, n) => join(run, folder, String(n).padStart(3,'0')+'.pgm');
const sourceTimeScale = 0.06875 / 0.050;   // full-domain physical metres / reference m/s
const trial = (label, folder, expected) => {
  const text = readFileSync(join(run, label+'.log'), 'utf8');
  const stats = readFileSync(join(run, label+'.stdout'), 'utf8').match(/Quadtree, (\d+) steps,/);
  assert.ok(stats, label+' completed');
  const raw = text.split('\n').filter(s => s.startsWith('frame,'));
  assert.equal(raw.length, expected, label);
  const frames = raw.map((s,i) => {
    const [index,t,volume,minFraction,maxFraction] = s.split(',').slice(1).map(Number);
    assert.equal(index,i);
    assert.ok([t,volume,minFraction,maxFraction].every(Number.isFinite));
    assert.ok(volume>0 && minFraction >= 0 && maxFraction <= 1);
    const timeSeconds=t*sourceTimeScale;
    assert.ok(Math.abs(timeSeconds-i/24)<1e-6,label+' physical time');
    return {index,t,timeSeconds,volume,minFraction,maxFraction};
  });
  assert.doesNotMatch(text,/warning|convergence for|failed|invalid VOF bounds/i);
  return {
    frames,summary:{states:expected,solverSteps:Number(stats[1]),initialFullDomainPhase:frames[0].volume,
      finalFullDomainPhase:frames.at(-1).volume,
      maxRelativeVolumeDrift:Math.max(...frames.map(f=>Math.abs(f.volume/frames[0].volume-1))),
      minVof:Math.min(...frames.map(f=>f.minFraction)),maxVof:Math.max(...frames.map(f=>f.maxFraction)),
      lastPhysicalTimeSeconds:frames.at(-1).timeSeconds,
      largestTimestampErrorSeconds:Math.max(...frames.map(f=>Math.abs(f.timeSeconds-f.index/24))),
      firstPgmSha256:sha(readFileSync(file(folder,0))),lastPgmSha256:sha(readFileSync(file(folder,expected-1))),
    },
  };
};
const specs = {
  front20:['grade20/cyan',96], front1000:['grade1000/cyan',96],
  rear20:['grade20/violet',96], rear1000:['grade1000/violet',96],
};
const trials=Object.fromEntries(Object.entries(specs).map(([id,[dir,n]])=>[id,trial(id,dir,n)]));
for(const [a,b] of [['front20','front1000'],['rear20','rear1000']])
  assert.equal(trials[a].summary.firstPgmSha256,trials[b].summary.firstPgmSha256);
assert.notEqual(trials.front20.summary.firstPgmSha256,trials.rear20.summary.firstPgmSha256);

const compare=(a,ta,b,tb)=>JSON.parse(execFileSync(measure,[file(specs[a][0],ta),file(specs[b][0],tb)],{encoding:'utf8'}));
const samples=[0,4,12,24,48,95];
const comparisons={};
for(const n of samples){
  comparisons[n]={physicalSeconds:n/24};
  for(const slide of ['front','rear']){
    comparisons[n][slide]={
      grades:compare(slide+'20',n,slide+'1000',n),
      lowVsInitial:compare(slide+'20',0,slide+'20',n),
      highVsInitial:compare(slide+'1000',0,slide+'1000',n),
    };
  }
}
const plane=704*704, frameBytes=plane*2;
function pack(grade) {
  const basename='pdms-water-'+grade+'cst.rg8.gz';
  const zip=readFileSync(join(assets,basename)), bytes=gunzipSync(zip);
  assert.equal(bytes.toString('ascii',0,8),'TSSDF002');
  const header=[704,704,96,24,96,208,512,288];
  header.forEach((v,i)=>assert.equal(bytes.readUInt32LE(8+4*i),v));
  assert.equal(bytes.length,40+96*frameBytes);
  const first=bytes.subarray(40,40+frameBytes);
  const firstChannelHash=[0,1].map(c=>{
    const single=Buffer.alloc(plane);
    for(let i=0;i<plane;i++)single[i]=first[2*i+c];
    return sha(single);
  });
  // The compact optical fields use a blurred signed distance. Compare the
  // thresholded result with the corresponding original coverage at the nearest
  // full-world cell centre, accounting for the field's different sampling grid.
  const registration={};
  const worst={rear:0,front:0};
  for(let n=0;n<96;n++){
    const channels=[['violet','rear'],['cyan','front']];
    const results=channels.map(([slide,name],channel)=>{
      const pgm=readFileSync(file('grade'+grade+'/'+slide,n));
      const raw=pgm.subarray(pgm.length-1024*1024);
      let disagreement=0;
      for(let y=208;y<496;y++)for(let x=96;x<608;x++){
        const i=Math.floor((y+.5)*1024/704)*1024+Math.floor((x+.5)*1024/704);
        const k=40+n*frameBytes+2*(y*704+x)+channel;
        if((raw[i]>=128)!==(bytes[k]>=128))disagreement++;
      }
      const fraction=disagreement/(512*288);
      assert.ok(fraction < .01, 'Source/cache gate mismatch '+grade+'/'+slide+'/'+n);
      worst[name]=Math.max(worst[name],fraction);
      return [name,fraction];
    });
    if([0,48,95].includes(n)) registration[n]=Object.fromEntries(results);
  }
  return {basename,compressedBytes:zip.length,gzipSha256:sha(zip),payloadSha256:sha(bytes),
    firstChannelHash,rawVsPackedGateXorFraction:registration,maximumRawVsPackedGateXorFraction:worst};
}
const caches=[pack(20),pack(1000)];
assert.deepEqual(caches[0].firstChannelHash,caches[1].firstChannelHash);
assert.notEqual(caches[0].firstChannelHash[0],caches[0].firstChannelHash[1]);
assert.notEqual(caches[0].payloadSha256,caches[1].payloadSha256);
const evidence={gateWidthMm:50,domainWidthMm:68.75,gapMm:2.5,referenceSpeedMmPerSecond:50,solverMaxLevel:7,
  fps:24,trials:Object.fromEntries(Object.entries(trials).map(([k,v])=>[k,v.summary])),
  comparisons,caches};
writeFileSync(join(output,'verified.json'),JSON.stringify(evidence,null,2)+'\n');
const csv=['trial,index,solverTime,timeSeconds,fullDomainPhase,minimumVof,maximumVof'];
for(const [name,run] of Object.entries(trials))for(const f of run.frames)
  csv.push([name,f.index,f.t,f.timeSeconds,f.volume,f.minFraction,f.maxFraction].join(','));
writeFileSync(join(output,'frames.csv'),csv.join('\n')+'\n');
