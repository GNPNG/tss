import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {join} from 'node:path';
const [stage,baseline] = process.argv.slice(2);
const source=await readFile(join(stage,'host.log'),'utf8');
const runs=source.split('\n').filter(line=>line.startsWith('{"variant"')).map(v=>JSON.parse(v));
if(runs.length!==2 || runs[0].variant!=='baseline' || runs[1].variant!=='working') throw Error('Expected both complete WKWebView sessions');
const quantile=(a,q)=>[...a].sort((x,y)=>x-y)[Math.max(0,Math.ceil(q*a.length)-1)];
const mean=a=>a.reduce((sum,x)=>sum+x,0)/a.length;
const round=v=>Math.round(v*1000)/1000;
function difference(a,b) {
  if (a.length!==b.length)throw Error('Mismatched frame lengths');
  let sum=0, max=0, changed=0;
  for(let i=0;i<a.length;i++) {const d=Math.abs(a[i]-b[i]);sum+=d;max=Math.max(max,d);changed+=d>16;}
  return {mean:sum/a.length,max,large:changed/a.length};
}
function metrics(row) {
  const frames=row.frames.map(s=>Buffer.from(s,'base64'));
  const motion=frames.slice(1).map((f,i)=>difference(f,frames[i]));
  if(motion.every(d=>d.mean===0))throw Error('Preview did not animate');
  return {
    scale:row.scale, intervalCount:row.intervals.length, callbackFps:round(1000/mean(row.intervals)),
    medianMs:round(quantile(row.intervals,.5)),p95Ms:round(quantile(row.intervals,.95)),maxMs:round(Math.max(...row.intervals)),
    meanCallbackMs:round(mean(row.callbackDuration)),over25ms:row.intervals.filter(v=>v>25).length,
    motionMedianMAE:round(quantile(motion.map(d=>d.mean),.5)),motionPeakMAE:round(Math.max(...motion.map(d=>d.mean))),
    peakFractionChannelsOver16:round(Math.max(...motion.map(d=>d.large))),
  };
}
const compared = runs[0].rows.map((r,i)=>{
  const current=runs[1].rows[i];
  const diffs=r.frames.map((s,j)=>difference(Buffer.from(s,'base64'),Buffer.from(current.frames[j],'base64')));
  return {scale:r.scale,meanAbsoluteRGBError:round(mean(diffs.map(d=>d.mean))), maximumChannelError:Math.max(...diffs.map(d=>d.max))};
});
const sha=async(path)=>createHash('sha256').update(await readFile(path)).digest('hex');
const reports=[];
for(const r of runs){
  const inputs={};
  for(const file of ['app.js','liquid-renderer.js','audio-features.js']) inputs[file]=await sha(join(stage,r.variant,file));
  reports.push({variant:r.variant, fileSha256:inputs, webglRenderer:r.renderer, webglContexts:r.contexts, rows:r.rows.map(metrics)});
}
console.log(JSON.stringify({baseline,preview:[1280,720],analysisImage:[96,54],simulatedSamples:18,stepSeconds:1/60,startSeconds:8,
   sourceOpacities:[1,.62,.47],audio:'unloaded',mode:'procedural',runs:reports,pixelComparison:compared}, null,2));
