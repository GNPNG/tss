// Observe the real GL framebuffer and the displayed canvas scanline in the same editor callback.
(() => {
  const variant = location.pathname.split('/')[1];
  const send = data => webkit.messageHandlers.cadence.postMessage(data);
  const fail = e => send('ERROR '+variant+' '+String(e?.message||e));
  window.addEventListener('error',e=>{if(e.error)fail(e.error);});
  window.addEventListener('unhandledrejection',e=>fail(e.reason));
  const raf=requestAnimationFrame.bind(window);
  const getContext=HTMLCanvasElement.prototype.getContext;
  const contexts=new Set(), rows=[];
  let tracing=false, draws=[], alphas=[], renderer=null;
  function strip(data) {
    const bytes=[];
    for(let x=8;x<1280;x+=16) bytes.push(...data.slice(x*4,x*4+3));
    return btoa(String.fromCharCode(...bytes));
  }
  HTMLCanvasElement.prototype.getContext=function(kind,...args) {
    const gl=getContext.call(this,kind,...args);
    if(gl && kind==='webgl' && !contexts.has(gl)) {
      contexts.add(gl);
      const ext=gl.getExtension('WEBGL_debug_renderer_info');
      renderer=ext?gl.getParameter(ext.UNMASKED_RENDERER_WEBGL):gl.getParameter(gl.RENDERER);
      const draw=gl.drawArrays.bind(gl);
      gl.drawArrays=(...args)=>{
        draw(...args);
        if(!tracing)return;
        const program=gl.getParameter(gl.CURRENT_PROGRAM);
        const value=k=>{const loc=gl.getUniformLocation(program,k);return loc===null?null:gl.getUniform(program,loc);};
        const y=gl.drawingBufferHeight-1-360;
        const buf=new Uint8Array(gl.drawingBufferWidth*4);
        gl.readPixels(0,y,gl.drawingBufferWidth,1,gl.RGBA,gl.UNSIGNED_BYTE,buf);
        draws.push({time:value('u_time'),scale:value('u_scale'),opacity:value('u_opacity'),
          colors:[0,1,2,3].map(i=>Array.from(value('u_color'+i))),scanline:strip(buf)});
      };
    }
    return gl;
  };
  const drawImage=CanvasRenderingContext2D.prototype.drawImage;
  CanvasRenderingContext2D.prototype.drawImage=function(...args){
    if(tracing && this.canvas.id==='preview')alphas.push(this.globalAlpha);
    return drawImage.call(this,...args);
  };
  const grab=document.createElement('canvas');grab.width=1280;grab.height=1;
  const read=grab.getContext('2d',{willReadFrequently:true});
  const $=id=>document.getElementById(id);
  const scales=[.4,1,2];
  const times=[...Array(8).fill(8),...Array.from({length:20},(_,i)=>8+i/60),...Array(4).fill(8)];
  const set=(id,v)=>{const el=$(id);el.value=String(v);for(const name of ['input','change'])el.dispatchEvent(new Event(name,{bubbles:true}));};
  let stage=0, idx=0, warm=0, frames=[], initialized=false, done=false;
  function setStage() {
    if(stage===0) for (const [i,a] of [1,.62,.47].entries()){
      if(i)$('add-layer').click();
      $('palette-grid').querySelectorAll('button')[[0,2,3][i]].click();set('opacity',a);
    }
    for(let i=0;i<3;i++){$('layer-list').querySelectorAll('.layer-row')[i].click();set('scale',scales[stage]);$('scale').dispatchEvent(new Event('pointerup'));}
    if(document.hidden || $('preview').width!==1280 || $('preview').height!==720)throw Error('Unexpected viewport');
    warm=0;idx=0;frames=[];
  }
  window.requestAnimationFrame=fn=>fn.name!=='renderFrame'?raf(fn):raf(timestamp=>{
    try {
      if(done)return;
      if(!initialized){setStage();initialized=true;}
      if(warm++<6){fn(8000);return;}
      draws=[];alphas=[];tracing=true;
      fn(times[idx]*1000);
      tracing=false;
      if (!$('render-error').hidden) throw Error($('render-error').textContent);
      read.clearRect(0,0,1280,1);
      read.drawImage($('preview'),0,360,1280,1,0,0,1280,1);
      const canvas=strip(read.getImageData(0,0,1280,1).data);
      frames.push({draws,alphas,canvas});
      if(++idx===times.length){
        rows.push({scale:scales[stage],frames});
        if(++stage===scales.length){done=true;send(JSON.stringify({variant,renderer,contexts:contexts.size,times,rows}));}
        else initialized=false;
      }
    } catch(e){done=true;fail(e);}
  });
})();
