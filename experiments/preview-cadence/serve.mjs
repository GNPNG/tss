import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {spawn} from 'node:child_process';
import {join, extname} from 'node:path';
const [, , stage, executable, mode] = process.argv;
if(!stage || !executable) throw Error('Expected staged assets directory and host executable');
const script=await readFile(new URL(mode==='diagnose'?'./diagnose.js':'./sample.js',import.meta.url),'utf8');
const server=createServer(async(req,res)=>{
  const match=/^\/(baseline|working)\/((?:assets\/)?[A-Za-z0-9._-]+)$/.exec(new URL(req.url,'http://localhost').pathname);
  if(!match) {res.writeHead(404).end();return;}
  try {
    const ext=extname(match[2]);
    const types={'.html':'text/html','.js':'application/javascript','.json':'application/json','.css':'text/css'};
    if(!types[ext]){res.writeHead(404).end();return;}
    const bytes=await readFile(join(stage,match[1],match[2]));
    res.setHeader('Content-Type',types[ext]);
    res.end(ext==='.html'?bytes.toString().replace('<head>','<head><script>'+script+'</script>'):bytes);
  } catch {res.writeHead(404).end();}
});
server.listen(0,'127.0.0.1',()=>{
  const proc=spawn(executable,[`http://127.0.0.1:${server.address().port}`],{stdio:'inherit'});
  proc.once('exit',(code)=>{server.close();process.exitCode=code??1;});
  proc.once('error',e=>{console.error(e);server.close();process.exitCode=1;});
});
