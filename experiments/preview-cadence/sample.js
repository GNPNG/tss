// Inserted before the unchanged editor scripts. Frames without instrumentation still use the platform's requestAnimationFrame.
(() => {
  const variant = location.pathname.split('/')[1];
  const send = message => webkit.messageHandlers.cadence.postMessage(message);
  const error = e => send('ERROR ' + variant + ' ' + String(e?.message || e));
  window.addEventListener('error', e => { if (e.error) error(e.error); });
  window.addEventListener('unhandledrejection', e => error(e.reason));
  const raf = requestAnimationFrame.bind(window);
  const context = HTMLCanvasElement.prototype.getContext;
  const contexts = new Set();
  let renderer = null;
  HTMLCanvasElement.prototype.getContext = function(type, ...args) {
    const gl = context.call(this, type, ...args);
    if (gl && type === 'webgl' && !contexts.has(gl)) {
      contexts.add(gl);
      const ext = gl.getExtension('WEBGL_debug_renderer_info');
      renderer = ext ? gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER);
    }
    return gl;
  };
  const scales = [0.4, 1, 2];
  const images = document.createElement('canvas');
  images.width=96; images.height=54;
  const read = images.getContext('2d',{willReadFrequently:true});
  const rows = [];
  let stage=0, phase='setup', warm=0, previous=0, imageStep=0, t0=0, actualTimes=[], callbackDuration=[], frames=[];
  const $ = id => document.getElementById(id);
  const set = (id,v) => { const el=$(id);el.value=String(v);el.dispatchEvent(new Event('input',{bubbles:true}));el.dispatchEvent(new Event('change',{bubbles:true})); };
  function setup() {
    if (stage===0) {
      for (const [i, opacity] of [1, .62, .47].entries()) {
        if(i) $('add-layer').click();
        $('palette-grid').querySelectorAll('button')[[0,2,3][i]].click();
        set('opacity', opacity);
      }
    }
    for(let i=0;i<3;i++) {
      $('layer-list').querySelectorAll('.layer-row')[i].click();
      set('scale',scales[stage]);
      $('scale').dispatchEvent(new Event('pointerup',{bubbles:true}));
    }
    const r=$('preview');
    if (r.width!==1280 || r.height!==720 || document.hidden || $('layer-list').children.length!==3)
      throw Error('Unexpected editor resolution, visibility or layer count');
  }
  const sampleImage = () => {
    read.clearRect(0,0,images.width,images.height);
    read.drawImage($('preview'),0,0,images.width,images.height);
    const pixels=read.getImageData(0,0,images.width,images.height).data;
    const rgb=new Uint8Array(images.width*images.height*3);
    for(let i=0,j=0;i<pixels.length;i+=4) {
      if (pixels[i+3]!==255) throw Error('The opaque base slide did not fill the sampled frame');
      rgb[j++]=pixels[i];rgb[j++]=pixels[i+1];rgb[j++]=pixels[i+2];
    }
    return btoa(String.fromCharCode(...rgb));
  };
  window.requestAnimationFrame = fn => {
    if(fn.name!=='renderFrame') return raf(fn);
    return raf(timestamp=>{
      try {
        if (phase==='setup') { setup();phase='warm';warm=0; }
        if (phase==='done') return;
        const motionTime = phase==='images' ? 8000 + imageStep*1000/60 : timestamp;
        const began=performance.now();
        fn(motionTime);
        const duration=performance.now()-began;
        if (!$('render-error').hidden) throw Error($('render-error').textContent);
        if (phase==='warm' && ++warm>=6) {
          t0=previous=timestamp;actualTimes=[];callbackDuration=[];phase='cadence';
        } else if (phase==='cadence') {
          actualTimes.push(timestamp-previous); callbackDuration.push(duration); previous=timestamp;
          if(timestamp-t0>=2200 && actualTimes.length>=6) { phase='images';imageStep=0;frames=[]; }
        } else if (phase==='images') {
          frames.push(sampleImage());
          if(++imageStep===18) {
            rows.push({scale:scales[stage],intervals:actualTimes,callbackDuration,frames});
            phase=++stage===scales.length?'done':'setup';
            if (phase==='done') send(JSON.stringify({variant,renderer,contexts:contexts.size,rows}));
          }
        }
      } catch(e) { phase='done';error(e); }
    });
  };
})();
