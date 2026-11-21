// Sample the complete sRGB gray axis through the generated link and the ECI
// output characterization. Use Argyll's independent ICC lookup implementation.
import {spawnSync} from 'node:child_process';
import {join} from 'node:path';
import assert from 'node:assert/strict';
const [program,profileDir]=process.argv.slice(2);
const colors=Array.from({length:1025},(_,n)=>Array(3).fill(n/1024).join(' ')).join('\n')+'\n';
function lookup(path,input,extra=[]){
  const r=spawnSync(program,['-v0','-ff',...extra,path],{input,encoding:'utf8'});
  assert.equal(r.status,0,r.stderr);
  return r.stdout;
}
const report={};
for(const [stock,destination] of [['coated','ISOcoated_v2_300_eci.icc'],['uncoated','PSO_Uncoated_ISO12647_eci.icc']]){
  const channels=lookup(new URL('../../live-wheel/assets/color/'+stock+'-gcr.icc',import.meta.url).pathname,colors);
  const labs=lookup(join(profileDir,destination),channels,['-ir','-pl']).trim().split(/\r?\n/).map(s=>s.trim().split(/\s+/).map(Number));
  assert.equal(labs.length,1025);
  const changes=labs.slice(1).map((p,i)=>p[0]-labs[i][0]);
  const peakNegative=Math.max(0,-Math.min(...changes));
  report[stock]={rampSamples:labs.length,blackL:labs[0][0],whiteL:labs.at(-1)[0],
    largestReversalL:peakNegative,reversalsAbove0_05L:changes.filter(d=>d<-.05).length};
}
console.log(JSON.stringify(report,null,2));
