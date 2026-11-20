// Invoke in a browser or desktop WebView with OfflineAudioContext available.
import { connectAudienceDelay, audienceDelaySeconds } from '../../WebAssets/projector-output.js';

export async function measureProgramDelay(sampleRate,delayMs,trimMs) {
  const delay=audienceDelaySeconds(delayMs,trimMs);
  const frames=Math.ceil((delay+.1)*sampleRate);
  const audio=new OfflineAudioContext(2,frames,sampleRate);
  const click=audio.createBuffer(1,frames,sampleRate);
  click.getChannelData(0)[32]=.75;
  const source=audio.createBufferSource();source.buffer=click;
  const merger=audio.createChannelMerger(2);
  source.connect(merger,0,0);
  const audience=audio.createGain();
  connectAudienceDelay(audio,source,audience,delay);
  audience.connect(merger,0,1);
  merger.connect(audio.destination);source.start();
  const result=await audio.startRendering();
  const cue=result.getChannelData(0),program=result.getChannelData(1);
  const strongest=a=>a.reduce((k,v,i)=>Math.abs(v)>Math.abs(a[k])?i:k,0);
  const cueSample=strongest(cue),audienceSample=strongest(program);
  const errorSamples=audienceSample-cueSample-Math.round(sampleRate*delay);
  if(Math.abs(errorSamples)>1 || Math.abs(program[audienceSample]-.75)>.005)
    throw new Error('Audience delay or impulse amplitude outside tolerance');
  return {sampleRate,delayMs,trimMs,cueSample,audienceSample,errorSamples};
}

