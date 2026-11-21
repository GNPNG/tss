import {readFile} from 'node:fs/promises';
import {join} from 'node:path';
const [stage]=process.argv.slice(2);
const runs=(await readFile(join(stage,'host.log'),'utf8')).split('\n').filter(s=>s.startsWith('{"variant"')).map(s=>JSON.parse(s));
if(runs.length!==2)throw Error('Expected both variants');
const bytes=s=>Buffer.from(s,'base64');
const mae=(a,b)=>a.reduce((s,x,i)=>s+Math.abs(x-b[i]),0)/a.length;
const worst=a=>Math.max(...a);
const round=v=>Math.round(v*1000)/1000;
function series(row){
  const pix=row.frames.map(f=>bytes(f.canvas));
  const repeats=pix.slice(1,8).map((p,i)=>mae(pix[i],p));
  const seek=pix.slice(-4).map(p=>mae(pix[0],p));
  const adjacent=pix.slice(9,28).map((p,i)=>mae(pix[8+i],p));
  return {fixedRepeatPeak:round(worst(repeats)),seekBackPeak:round(worst(seek)),stepPeak:round(worst(adjacent))};
}
function blend(f) {
  const layers=f.draws.map(d=>bytes(d.scanline));
  return layers.reduce((lower,upper,i)=>upper.map((v,k)=>Math.round(v*f.alphas[i]+lower[k]*(1-f.alphas[i]))),Buffer.alloc(240));
}
const out=runs[0].rows.map((r,i)=>{
  const s=runs[1].rows[i];
  if(s.scale!==r.scale)throw Error('Scale mismatch');
  const submittedMatch=r.frames.every((f,j)=>f.draws.length===3 && s.frames[j].draws.length===3 && f.draws.every((d,k)=>{
    const c=s.frames[j].draws[k];
    return d.time===c.time && d.scale===c.scale && Math.abs(f.alphas[k]-c.opacity)<1e-5 && JSON.stringify(d.colors)===JSON.stringify(c.colors);
  }));
  const peakPerFrame=(fn)=>round(worst(r.frames.map((f,j)=>fn(f,s.frames[j]))));
  return {scale:r.scale,matchingShaderControls:submittedMatch,
    submitted:{baseline:r.frames[0].alphas, working:s.frames[0].draws.map(d=>d.opacity), fixedTime:r.frames[0].draws.map(d=>d.time)},
    baseline:series(r),working:series(s),
    baselineCanvasVsReferenceBlendPeakMAE:peakPerFrame(f=>mae(blend(f),bytes(f.canvas))),
    workingCanvasVsFinalFramebufferPeakMAE:peakPerFrame((_,f)=>mae(bytes(f.draws[2].scanline),bytes(f.canvas))),
    oldVsNewOpaqueBaseFramebufferPeakMAE:peakPerFrame((f,g)=>mae(bytes(f.draws[0].scanline),bytes(g.draws[0].scanline))),
    oldReferenceBlendVsNewFramebufferPeakMAE:peakPerFrame((f,g)=>mae(blend(f),bytes(g.draws[2].scanline))),
    oldVsNewCanvasPeakMAE:peakPerFrame((f,g)=>mae(bytes(f.canvas),bytes(g.canvas))),
  };
});
console.log(JSON.stringify({sampling:'80 RGB pixels across row 360, synchronously after each draw', framesPerScale:32,rows:out},null,2));
