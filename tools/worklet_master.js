class LoomSat extends AudioWorkletProcessor{
static get parameterDescriptors(){return[{name:'drive',defaultValue:0.3,minValue:0,maxValue:1,automationRate:'k-rate'}];}
constructor(){super();this.z=[0,0];}
process(inputs,outputs,p){const ins=inputs[0],outs=outputs[0];if(!ins||!ins.length)return true;
const d=p.drive[0],g=1+d*4,comp=1/(1+d*0.6);
for(let ch=0;ch<outs.length;ch++){const x=ins[ch]||ins[0],y=outs[ch];let prev=this.z[ch]||0;
for(let i=0;i<y.length;i++){const s0=(prev+x[i])*0.5;y[i]=(Math.tanh(s0*g)+Math.tanh(x[i]*g))*0.5*comp;prev=x[i];}
this.z[ch]=prev;}return true;}}
registerProcessor('loom-sat',LoomSat);
class LoomLim extends AudioWorkletProcessor{
static get parameterDescriptors(){return[{name:'ceiling',defaultValue:0.97,minValue:0.1,maxValue:1,automationRate:'k-rate'}];}
constructor(){super();this.la=128;this.b=[new Float32Array(this.la),new Float32Array(this.la)];this.pos=0;this.g=1;}
process(inputs,outputs,p){const ins=inputs[0],outs=outputs[0];if(!ins||!ins.length)return true;
const ceil=p.ceiling[0],n=outs[0].length,ch=outs.length;
for(let i=0;i<n;i++){let peak=0;for(let c=0;c<ch;c++){const v=Math.abs((ins[c]||ins[0])[i]);if(v>peak)peak=v;}
const tg=peak>ceil?ceil/peak:1;if(tg<this.g)this.g=tg;else this.g+=(1-this.g)*0.0009;
for(let c=0;c<ch;c++){const x=(ins[c]||ins[0])[i],dd=this.b[c][this.pos];this.b[c][this.pos]=x;outs[c][i]=dd*this.g;}
this.pos=(this.pos+1)%this.la;}return true;}}
registerProcessor('loom-lim',LoomLim);
class LoomRedux extends AudioWorkletProcessor{
static get parameterDescriptors(){return[{name:'reduce',defaultValue:0.5,minValue:0,maxValue:1,automationRate:'k-rate'}];}
constructor(){super();this.h=[0,0];this.ph=0;}
process(inputs,outputs,p){const ins=inputs[0],outs=outputs[0];if(!ins||!ins.length)return true;
const f=1+p.reduce[0]*30,n=outs[0].length,ch=outs.length;
for(let i=0;i<n;i++){this.ph+=1;if(this.ph>=f){this.ph-=f;for(let c=0;c<ch;c++)this.h[c]=(ins[c]||ins[0])[i];}
for(let c=0;c<ch;c++)outs[c][i]=this.h[c];}return true;}}
registerProcessor('loom-redux',LoomRedux);
class LoomReson extends AudioWorkletProcessor{
static get parameterDescriptors(){return[{name:'freq',defaultValue:220,minValue:20,maxValue:8000,automationRate:'k-rate'},{name:'decay',defaultValue:0.92,minValue:0,maxValue:0.995,automationRate:'k-rate'}];}
constructor(){super();this.b=[new Float32Array(4096),new Float32Array(4096)];this.pos=0;this.lp=[0,0];}
process(inputs,outputs,p){const ins=inputs[0],outs=outputs[0];if(!ins||!ins.length)return true;
const per=Math.max(2,Math.min(4095,Math.round(sampleRate/p.freq[0]))),dec=p.decay[0],n=outs[0].length,ch=outs.length;
for(let i=0;i<n;i++){for(let c=0;c<ch;c++){const x=(ins[c]||ins[0])[i],rd=(this.pos-per+4096)%4096,fb=this.b[c][rd];
this.lp[c]=this.lp[c]*0.5+fb*0.5;const y=x+this.lp[c]*dec;this.b[c][this.pos]=y;outs[c][i]=y*0.5;}this.pos=(this.pos+1)%4096;}return true;}}
registerProcessor('loom-reson',LoomReson);
class LoomFlange extends AudioWorkletProcessor{
static get parameterDescriptors(){return[{name:'rate',defaultValue:0.3,minValue:0.01,maxValue:10,automationRate:'k-rate'},{name:'depth',defaultValue:0.6,minValue:0,maxValue:1,automationRate:'k-rate'}];}
constructor(){super();this.b=[new Float32Array(2048),new Float32Array(2048)];this.pos=0;this.ph=0;}
process(inputs,outputs,p){const ins=inputs[0],outs=outputs[0];if(!ins||!ins.length)return true;
const rate=p.rate[0],depth=p.depth[0],n=outs[0].length,ch=outs.length;
for(let i=0;i<n;i++){this.ph+=rate*6.2831853/sampleRate;const lfo=Math.sin(this.ph)*0.5+0.5,ds=2+lfo*depth*60;
for(let c=0;c<ch;c++){const x=(ins[c]||ins[0])[i],ri=this.pos-ds,r0=Math.floor(ri),fr=ri-r0,
a=this.b[c][((r0%2048)+2048)%2048],bb=this.b[c][(((r0+1)%2048)+2048)%2048],dl=a+(bb-a)*fr;
this.b[c][this.pos]=x+dl*0.5;outs[c][i]=(x+dl)*0.6;}this.pos=(this.pos+1)%2048;}return true;}}
registerProcessor('loom-flange',LoomFlange);
class LoomWidth extends AudioWorkletProcessor{
static get parameterDescriptors(){return[{name:'width',defaultValue:1.4,minValue:0,maxValue:2,automationRate:'k-rate'}];}
process(inputs,outputs,p){const ins=inputs[0],outs=outputs[0];if(!ins||!ins.length)return true;const w=p.width[0],n=outs[0].length;
if(outs.length<2||ins.length<2){for(let c=0;c<outs.length;c++){const x=ins[c]||ins[0];for(let i=0;i<n;i++)outs[c][i]=x[i];}return true;}
const l=ins[0],r=ins[1];for(let i=0;i<n;i++){const m=(l[i]+r[i])*0.5,s=(l[i]-r[i])*0.5*w;outs[0][i]=m+s;outs[1][i]=m-s;}return true;}}
registerProcessor('loom-width',LoomWidth);
