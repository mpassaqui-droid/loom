# Mesure la gigue du port MIDI virtuel « LOOM » (horodatage CoreMIDI, rtmidi).
# Usage : LOOM_AUDIO_DEVICE=BlackHole ./target/release/loom play tools/gigue.loom &  python3 tools/midi_gigue.py 12
# Affiche les trous (intervalle != 125 ms) et le nombre de notes.
import rtmidi, time, sys
m=rtmidi.MidiIn()
while 'LOOM' not in m.get_ports(): time.sleep(0.05)
m.open_port(m.get_ports().index('LOOM')); ev=[]; clock=0.0; t0=time.time()
while time.time()-t0<float(sys.argv[1]):
    r=m.get_message()
    if r: clock+=r[1]; ev.append((round(clock*1000,1),r[0]))
    else: time.sleep(0.001)
on=[(t,x[1]) for t,x in ev if x[0]&0xF0==0x90 and x[2]>0]
for (a,p),(b,q) in zip(on,on[1:]):
    if abs(b-a-125)>4: print('trou', a, p, '->', b, q)
print('on',len(on),'msgs',len(ev))
