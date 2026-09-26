# REPRISE — LOOM P6-3 Pierre A

**Mot de reprise : « P6-3A ».** Repars d'ici.

ÉTAT : branche `p6-3a-wip` (commit a8c99ab) au-dessus de main `2dfcd97`.
- Rust render.rs (14 verbes riches) = DONE & testé (160/160, clippy 0).
- loom-wasm/src/lib.rs = DONE dans le WIP : SynthV1 étendu + synth_v1() sérialisent
  fm=Option<[ratio,index,fb]>, partials=Option<Vec<f32>>, wt=Option<String> (nom brut),
  pwm/sync/noise/drive/fold = f32 déjà RÉSOLUS (mêmes clamps que NoteParam::of),
  am/ring = f32 bruts, bell=bool, crush=Option<u32> (bits bruts), redux=f32 (0..1 brut).

RESTE (dans l'ordre) :
1) MIROIR JS dans web/index.html — étendre `noteSample` (PARTAGÉ renderPCM+renderFX,
   ~ligne 7216) op-pour-op avec render.rs note_sample/osc_value/apply_inserts.
   Ajouter helpers F=fround près des autres (~7170) : bellValue(t,f0)
   [R=[1,1.96,2.92,4.2] A=[1,.6,.3,.15] TAU=[1.5,1.2,1,.8], Σ A·exp(-t/τ)·sinPi(R·f0·t)],
   partialsValue(arr,ph) [Σ a_i·sinPi((i+1)·ph)], wtValue(amps,sum,ph) [même Σ /max(1e-6,sum)],
   noiseValue(c,sr) [xorshift32 : s^=s<<13; s^=s>>>17; s^=s<<5, tout >>>0 ;
   white=F(F((s>>>16)*F(1/32768))-1) ; alpha=1-Math.exp(-2π·6000/sr) ; c.lp+=alpha*(white-c.lp) ; ret white-c.lp],
   uniFk(st,nUni,k,f0), oscValue(c,st,k,fk,sr,wf) [FM lit fm_ph/fm_prev PARTAGÉS entre voix,
   avance porteuse *ph+=(fk+index·m)/sr puis fm_ph+=(ratio·fk)/sr, return wave(*ph NEUF) ;
   sinon mnew=wrap(ph+fk/sr), wrapped=mnew<ph ; sync>0 → slave (=mnew si wrapped sinon wrap(slave+fk·sync/sr)) ;
   sinon wt ; sinon partials ; sinon pwm [pw=0.12+N·(0.5+0.5·sinPi(pwm_ph)), ph<pw?1:-1] ;
   sinon wave(ph) ; *ph=mnew à la fin],
   applyInserts(c,st,i,x) [drive tanh g=1+4N comp=1/(1+.6N) ; fold 1-|((x+1)%4+4)%4 -2| mix ;
   crush v=x·scale, r=Math.sign(v)·Math.round(|v|) (half-away!), x=r/scale ; redux i%K===0→held].
   ORDRE dans noteSample : si bell→out=bellValue + on avance quand même les uni_ph ;
   sinon Σ oscValue /nUni ; puis (si pwm) avancer pwm_ph=wrap(pwm_ph+(f0/4)/sr) ; puis noise ;
   sub ; double ; am [out*=0.5+0.5·sinPi(am_ph)] ; ring [out*=sinPi(ring_ph)] ; SVF ; applyInserts.
   Étendre les 2 blocs `st` (renderPCM ~7291 ET renderFX ~7358) : lire les nouveaux champs,
   st.crushScale=F(2^(max(1,bits)-1)) si crush!=null, st.redux=round(1+20·clamp(redux,0,1)),
   st.wtAmps/wtSum via table {organ,warm,rich,soft} (sinon null), st.fm/partials/pwm/sync/bell/noise/am/ring/drive/fold.
   Étendre le contexte `c` aux 2 endroits (~7308 ET ~7397) : ajouter
   fmPh:0,fmPrev:0,pwmPh:0,slave:[0…nUni],ns:noiseSeed(start),lp:0,amPh:0,ringPh:0,held:0
   où noiseSeed=((start>>>0)+0x9E3779B9)>>>0 (start = celui déjà calculé pour la note).
   Tables wt (=render.rs) : organ[1,1,1,1,1,.9,.7,.5,.3] warm[1,.5,.25,.15,.1,.06,.03,.02,.01]
   rich[1,.9,.8,.7,.6,.5,.4,.3,.2] soft[1,.5,.4,.3,.2,.15,.1,.08,.05] ; sum = Σ (fold f32).
2) ./build-wasm.sh (régénère web/pkg + tools/pkg-node).
3) Corpus RENDER2 dans tools/parity_test.js (objet `rf_pieces` ~ligne 247) : AJOUTER ~6 pièces
   fm, wt organ, pwm, noise, drive/fold/crush/redux, bell — sans drums (silence). Gate 5e-3, WAV ≤1 step.
4) PORTES : cargo test --workspace ; cargo clippy --workspace -- -D warnings ;
   node tools/check_web.js ; node tools/parity_test.js ; git diff --check.
5) Puis basculer sur main et y faire les 2 vrais commits FR (repartir du WIP :
   cherry-pick a8c99ab ou fast-forward) : commit 1 = wasm+miroir+parité
   "P6-3A · … miroir JS + parité" ; commit 2 = todo.md (cocher Pierre A) + maj grille.
   Push quand l'owner relance. Supprimer la branche WIP + ce fichier après.

RÈGLES D'IDENTITÉ : F=Math.fround, produits f32 alignés, off-by-one phases (Osc lit AVANT
d'avancer, sauf FM qui lit *ph NEUF), round() Rust=half-away → JS via Math.sign. Bug connu :
bytes.set JS exige des offsets explicites. Rust = autorité, JS = repli op-pour-op.

VÉRIFIÉ cette session : detune/sub/double/SVF/ADSR/`start` déjà mirrorés (parité P6-1) ;
render_fx.rs (l.94/106) seed le bruit comme render.rs et appelle le même note_sample →
étendre l'unique noteSample JS couvre renderPCM ET renderFX.
</content>
</invoke>
