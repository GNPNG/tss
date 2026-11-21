// Pixel coordinates match the delivery specification and the clean optical masters.
export const plateFormats = {
  visualizer: {size:[3840,2160], colors:['rec709','display-p3']},
  'dsp-cover': {size:[4000,4000], colors:['srgb']},
  vertical: {size:[2160,3840], colors:['srgb','rec709']},
  panorama: {size:[10800,2160], colors:['display-p3','srgb']},
};
const colorNames = {rec709:'Rec.709',srgb:'sRGB IEC 61966-2-1','display-p3':'Display P3'};
const ns = 'http://www.w3.org/2000/svg';

export class PlateFormatControls {
  constructor(root, capture, status, geometry) {
    this.root=root;
    this.geometry=geometry;
    this.status=status;
    this.stock=root.querySelector('#plate-stock');
    this.layout=root.querySelector('#plate-layout');
    this.color=root.querySelector('#plate-color');
    this.horizon=root.querySelector('#plate-horizon');
    this.y=root.querySelector('#plate-horizon-y');
    this.guide=root.querySelector('svg');
    this.source=null;
    this.layout.addEventListener('change', () => this.render(true));
    this.horizon.addEventListener('change',()=>this.render());
    this.y.addEventListener('input',()=>this.render());
    const preview=root.querySelector('#plate-frame');
    preview.addEventListener('click',async()=>{
      preview.disabled=true;
      try {
        this.source='data:image/png;base64,'+await capture();
        this.render();
      } catch(e) { status('Could not preview framing: '+String(e)); }
      finally { preview.disabled=false; }
    });
    this.render(true);
  }

  options() {
    if(!plateFormats[this.layout.value]){
      const catalog=this.root.querySelector('#print-catalog').value;
      return {print:{layout:this.layout.value,stock:this.stock.value,catalog:catalog?Number(catalog):null,
        gilding:this.root.querySelector('#print-spot').value||null,richBlack:this.root.querySelector('#print-rich').checked}};
    }

    const y=this.y.valueAsNumber;
    if(this.layout.value==='visualizer' && this.horizon.checked &&
       (!Number.isInteger(y)||y<0||y>2159)) throw Error('Horizon must be a pixel row from 0 to 2159.');
    return {layout:this.layout.value,encoding:this.color.value,
      horizonY:this.layout.value==='visualizer' && this.horizon.checked ? y : null};
  }

  async renderPrint(kind) {
    const {spec,canvasMm:[w,h],guide}=await this.geometry(kind);
    if(this.layout.value!==kind) return;
    this.guide.replaceChildren();
    this.guide.setAttribute('viewBox',[0,0,w,h].join(' '));
    this.guide.style.aspectRatio=w+'/'+h;
    const image=(href,preserve)=>{
      const node=document.createElementNS(ns,'image');
      for(const [key,value] of Object.entries({href,width:w,height:h,preserveAspectRatio:preserve})) node.setAttribute(key,value);
      this.guide.append(node);
    };
    if(this.source) image(this.source,'xMidYMid slice');
    image('data:image/svg+xml;charset=utf-8,'+encodeURIComponent(guide),'none');
    this.root.querySelector('figcaption').textContent=spec.pixels.join(' × ')+' px at 300 ppi · '+w+' × '+h+' mm with '+spec.bleedMm+' mm bleed · '+spec.safeMm+' mm type safety. Red die, green safe, purple fold. Choose the original ECI stock profile when exporting.';
  }

  render(resetColor=false) {
    const kind=this.layout.value;
    const digital=plateFormats[kind];
    this.root.querySelector('#plate-color-label').hidden=!digital;
    this.root.querySelector('#plate-stock-label').hidden=!!digital;
    this.root.querySelector('#print-finishing').hidden=!!digital;
    if(!digital){
      this.root.querySelector('#plate-horizon-controls').hidden=true;
      this.renderPrint(kind).catch(e=>this.status('Could not load the mechanical: '+String(e)));
      return;
    }
    const {size:[w,h],colors}=digital;

    if(resetColor) this.color.replaceChildren(...colors.map(id=>new Option(colorNames[id],id)));
    this.root.querySelector('#plate-horizon-controls').hidden=kind!=='visualizer';
    this.y.disabled=!this.horizon.checked;
    this.guide.replaceChildren();
    this.guide.setAttribute('viewBox', [0,0,w,h].join(' '));
    this.guide.style.aspectRatio=w+'/'+h;
    const append=(name,attrs)=>{
      const node=document.createElementNS(ns,name);
      for(const [k,v] of Object.entries(attrs)) node.setAttribute(k,String(v));
      this.guide.append(node);
    };
    if(this.source) append('image',{href:this.source,width:w,height:h,preserveAspectRatio:'xMidYMid slice'});
    const mark={stroke:'white','stroke-width':1.5,'vector-effect':'non-scaling-stroke',fill:'none','stroke-dasharray':'7 5'};
    const line=(x1,y1,x2,y2)=>append('line',{x1,y1,x2,y2,...mark});
    let caption=w+' × '+h+' px';
    if(kind==='vertical'){
      append('rect',{x:0,y:576,width:w,height:499.2,...mark});append('rect',{x:0,y:3072,width:w,height:768,...mark,fill:'black','fill-opacity':0.3});
      caption+=' · Title and sigil at 15–28%. Lower 20% reserved for platform UI.';
    } else if(kind==='panorama'){
      for(let i=1;i<5;i++) line(2160*i,0,2160*i,h);
      caption+=' · Five adjoining 2160 px panels, EPNI-006 through EPNI-010.';
    } else if(kind==='visualizer' && this.horizon.checked && Number.isInteger(this.y.valueAsNumber)){
      line(0,this.y.valueAsNumber,w,this.y.valueAsNumber);
      caption+=' · Horizon '+this.y.value+' px from the top.';
    }
    this.root.querySelector('figcaption').textContent=caption;
  }
}
