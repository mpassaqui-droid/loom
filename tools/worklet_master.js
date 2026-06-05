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
