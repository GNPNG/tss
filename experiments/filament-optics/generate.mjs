// Generate simple, independently selectable optical test plates as glTF.
// These are static calibration fixtures, not simulated liquids.
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const destination = process.argv[2];
if (!destination) throw new Error('Usage: node generate.mjs OUTPUT_DIRECTORY');
await mkdir(destination, { recursive: true });

function makeScene(front, rear) {
  const meshes = [], nodes = [];
  const materials = [
    { name: 'checker-light', extensions: { KHR_materials_unlit: {} }, pbrMetallicRoughness: { baseColorFactor: [0.75,0.75,0.75,1], metallicFactor: 0, roughnessFactor: 1 } },
    { name: 'checker-dark', extensions: { KHR_materials_unlit: {} }, pbrMetallicRoughness: { baseColorFactor: [0.08,0.08,0.08,1], metallicFactor: 0, roughnessFactor: 1 } },
    ...[[0.05,0.75,0.85],[0.8,0.08,0.55]].map((color, i) => ({
      name: i ? 'magenta-rear' : 'cyan-front', doubleSided: false,
      pbrMetallicRoughness: { baseColorFactor: [1,1,1,1], metallicFactor: 0, roughnessFactor: 0.02 },
      extensions: { KHR_materials_transmission: { transmissionFactor: 1 }, KHR_materials_ior: { ior: 1.4 }, KHR_materials_volume: { thicknessFactor: 0.1, attenuationDistance: 0.2, attenuationColor: color } },
    })),
  ];
  const groups = new Map();
  function quad(x0,y0,x1,y1,z,material) {
    let g = groups.get(material); if (!g) groups.set(material, g=[]);
    const p = [[x0,y0,z],[x1,y0,z],[x1,y1,z],[x0,y1,z]];
    g.push(p);
  }
  function box(x0,y0,x1,y1,z0,z1,material) {
    let g = groups.get(material); if (!g) groups.set(material, g=[]);
    g.push(
      [[x0,y0,z1],[x1,y0,z1],[x1,y1,z1],[x0,y1,z1]],
      [[x1,y0,z0],[x0,y0,z0],[x0,y1,z0],[x1,y1,z0]],
      [[x0,y0,z0],[x0,y0,z1],[x0,y1,z1],[x0,y1,z0]],
      [[x1,y0,z1],[x1,y0,z0],[x1,y1,z0],[x1,y1,z1]],
      [[x0,y1,z1],[x1,y1,z1],[x1,y1,z0],[x0,y1,z0]],
      [[x0,y0,z0],[x1,y0,z0],[x1,y0,z1],[x0,y0,z1]],
    );
  }
  for (let y=0;y<10;y++) for (let x=0;x<10;x++) quad(-1+x*.2,-1+y*.2,-.8+x*.2,-.8+y*.2,-.35,(x+y)%2);
  if (rear) box(-.82,-.7,.35,.7,-.15,-.05,3);
  if (front) box(-.35,-.7,.82,.7,.05,.15,2);
  const accessorDefinitions = [], bufferViews=[];
  for (const [material, quads] of groups) {
    const pos=[], norm=[], idx=[];
    for (const q of quads) {
      const k=pos.length/3;
      const a=q[1].map((v,i)=>v-q[0][i]), b=q[2].map((v,i)=>v-q[0][i]);
      const n=[a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]];
      const length=Math.hypot(...n);for(const v of q){pos.push(...v);norm.push(...n.map(c=>c/length));}
      idx.push(k,k+1,k+2,k,k+2,k+3);
    }
    const min=[0,1,2].map(i=>Math.min(...pos.filter((_,n)=>n%3===i)));
    const max=[0,1,2].map(i=>Math.max(...pos.filter((_,n)=>n%3===i)));
    const pi=accessorDefinitions.length;accessorDefinitions.push({kind:'f',values:pos,type:'VEC3',count:pos.length/3,min,max,target:34962});
    const ni=accessorDefinitions.length;accessorDefinitions.push({kind:'f',values:norm,type:'VEC3',count:norm.length/3,target:34962});
    const ii=accessorDefinitions.length;accessorDefinitions.push({kind:'u',values:idx,type:'SCALAR',count:idx.length,target:34963});
    meshes.push({primitives:[{attributes:{POSITION:pi,NORMAL:ni},indices:ii,material}]});nodes.push({mesh:meshes.length-1});
  }
  let offset=0;const chunks=[];const accessors=[];
  for(const a of accessorDefinitions){
    const b=Buffer.alloc(a.values.length*(a.kind==='f'?4:2));
    a.values.forEach((v,i)=>a.kind==='f'?b.writeFloatLE(v,i*4):b.writeUInt16LE(v,i*2));
    const view=bufferViews.length;bufferViews.push({buffer:0,byteOffset:offset,byteLength:b.length,target:a.target});
    accessors.push({bufferView:view,componentType:a.kind==='f'?5126:5123,count:a.count,type:a.type,...(a.min?{min:a.min,max:a.max}:{})});
    chunks.push(b);offset+=b.length;const pad=(4-offset%4)%4;if(pad){chunks.push(Buffer.alloc(pad));offset+=pad;}
  }
  const buffer=Buffer.concat(chunks);
  return {asset:{version:'2.0',generator:'TSS optical validation fixture'},extensionsUsed:['KHR_materials_unlit','KHR_materials_transmission','KHR_materials_ior','KHR_materials_volume'],scene:0,scenes:[{nodes:nodes.map((_,i)=>i)}],nodes,meshes,materials,accessors,bufferViews,buffers:[{byteLength:buffer.length,uri:'data:application/octet-stream;base64,'+buffer.toString('base64')}]};
}

for (const [name, front, rear] of [['clear',false,false],['front',true,false],['rear',false,true],['overlap',true,true]]) {
  await writeFile(join(destination, `${name}.gltf`),JSON.stringify(makeScene(front,rear)));
}
