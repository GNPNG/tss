// Session-local decoder handles never enter a portable project or track template.
const mediaKeys = ['name','sha256','durationUs','width','height','still','offsetUs','rate','loop','mode',
  'keyEnabled','keyColor','keyThreshold','keySoftness','gradientA','gradientB','gradientRate','bounce','blur','distortion',
  'maskA','maskB','maskOp','shape'];

export function portableMediaSettings(media) {
  return Object.fromEntries(mediaKeys.filter(k=>media[k]!==undefined).map(k=>[k,media[k]]));
}
export function bindProjectMedia(layer, grant) {
  if(!layer.media || layer.media.sha256!==grant.sha256)throw Error('Visual media fingerprint does not match the project.');
  Object.assign(layer.media,{assetId:grant.id,width:grant.width,height:grant.height,durationUs:grant.durationUs,still:grant.still});
}
export function validMediaSettings(m) {
  const range=(v,lo,hi)=>Number.isFinite(v)&&v>=lo&&v<=hi;
  const uint=v=>Number.isSafeInteger(v)&&v>=0;
  const color=v=>typeof v==='string'&&/^#[0-9a-f]{6}$/i.test(v);
  const mask=v=>v==null||(uint(v)&&v!==0);
  return !!m && typeof m==='object' && typeof m.name==='string' && m.name.length>0 && !/[\\/\u0000]/.test(m.name) &&
    typeof m.sha256==='string'&&/^[0-9a-f]{64}$/.test(m.sha256) && uint(m.durationUs) && uint(m.width) && m.width>0 &&
    uint(m.height) && m.height>0 && typeof m.still==='boolean' && uint(m.offsetUs) && range(m.rate,0,8) && typeof m.loop==='boolean' &&
    ['color','key','silhouette','gradient'].includes(m.mode) && (m.keyEnabled===undefined||typeof m.keyEnabled==='boolean') &&
    [m.keyColor,m.gradientA,m.gradientB].every(color) &&
    [m.keyThreshold,m.keySoftness,m.bounce,m.blur,m.distortion].every(v=>range(v,0,1)) && range(m.gradientRate,0,12) &&
    mask(m.maskA) && mask(m.maskB) && ['intersect','union','subtract','xor'].includes(m.maskOp) &&
    ['none','circle','diamond','star'].includes(m.shape);
}
