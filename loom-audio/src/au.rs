//! Héberger des Audio Units (macOS) : prototype.
//!
//! LOOM peut lister les AU installés (instruments et effets) et jouer des notes
//! dans un instrument AU, hors du fil audio (rendu) : `loom plugins`,
//! `loom au "Nom" fichier.loom sortie.wav`. Pas de dépendance : AudioToolbox
//! directement, le même chemin que Logic ou Live pour un AU v2.
//!
//! Limites assumées du prototype : instruments seulement pour le rendu (un effet
//! se liste mais ne traite pas encore de son), pas de fenêtre du plug-in, pas
//! d'état sauvegardé. Le VST3 et Ableton Link sont étudiés dans `docs/plugins-link.md`.

use std::ffi::c_void;

type OSStatus = i32;
type AudioComponent = *mut c_void;
type AudioUnit = *mut c_void;
type CFStringRef = *const c_void;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Desc {
    kind: u32,
    sub: u32,
    manu: u32,
    flags: u32,
    mask: u32,
}

#[repr(C)]
struct Asbd {
    rate: f64,
    format: u32,
    flags: u32,
    bytes_per_packet: u32,
    frames_per_packet: u32,
    bytes_per_frame: u32,
    channels: u32,
    bits: u32,
    reserved: u32,
}

#[repr(C)]
#[derive(Default)]
struct Smpte {
    subframes: i16,
    divisor: i16,
    counter: u32,
    kind: u32,
    flags: u32,
    hours: i16,
    minutes: i16,
    seconds: i16,
    frames: i16,
}

#[repr(C)]
#[derive(Default)]
struct TimeStamp {
    sample_time: f64,
    host_time: u64,
    rate_scalar: f64,
    word_clock: u64,
    smpte: Smpte,
    flags: u32,
    reserved: u32,
}

#[repr(C)]
struct Buffer {
    channels: u32,
    bytes: u32,
    data: *mut c_void,
}

#[repr(C)]
struct BufferList2 {
    count: u32,
    buffers: [Buffer; 2],
}

#[link(name = "AudioToolbox", kind = "framework")]
extern "C" {
    fn AudioComponentFindNext(c: AudioComponent, d: *const Desc) -> AudioComponent;
    fn AudioComponentCopyName(c: AudioComponent, name: *mut CFStringRef) -> OSStatus;
    fn AudioComponentGetDescription(c: AudioComponent, d: *mut Desc) -> OSStatus;
    fn AudioComponentInstanceNew(c: AudioComponent, out: *mut AudioUnit) -> OSStatus;
    fn AudioComponentInstanceDispose(u: AudioUnit) -> OSStatus;
    fn AudioUnitSetProperty(u: AudioUnit, id: u32, scope: u32, elem: u32, data: *const c_void, size: u32) -> OSStatus;
    fn AudioUnitInitialize(u: AudioUnit) -> OSStatus;
    fn AudioUnitUninitialize(u: AudioUnit) -> OSStatus;
    fn AudioUnitRender(u: AudioUnit, flags: *mut u32, ts: *const TimeStamp, bus: u32, frames: u32, data: *mut BufferList2) -> OSStatus;
    fn MusicDeviceMIDIEvent(u: AudioUnit, status: u32, d1: u32, d2: u32, offset: u32) -> OSStatus;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringGetCString(s: CFStringRef, buf: *mut u8, len: isize, enc: u32) -> bool;
    fn CFRelease(p: *const c_void);
}

const fn fourcc(s: &[u8; 4]) -> u32 { u32::from_be_bytes(*s) }

const INSTRUMENT: u32 = fourcc(b"aumu");
const EFFECT: u32 = fourcc(b"aufx");
const MUSIC_EFFECT: u32 = fourcc(b"aumf");
const PROP_STREAM_FORMAT: u32 = 8;
const PROP_MAX_FRAMES: u32 = 14;
const SCOPE_GLOBAL: u32 = 0;
const SCOPE_OUTPUT: u32 = 2;
/// Le plus de trames demandées en une fois à l'AU.
const SLICE: usize = 512;

/// Un Audio Unit installé.
#[derive(Debug, Clone, PartialEq)]
pub struct AuInfo {
    /// « Fabricant: Nom », comme l'affiche macOS.
    pub name: String,
    /// `true` pour un instrument (il reçoit des notes), `false` pour un effet.
    pub instrument: bool,
    desc: (u32, u32, u32),
}

fn cf_string(s: CFStringRef) -> String {
    if s.is_null() { return String::new(); }
    let mut buf = [0u8; 512];
    // SAFETY : `s` est une CFString valide rendue par AudioComponentCopyName ; le tampon est borné.
    let ok = unsafe { CFStringGetCString(s, buf.as_mut_ptr(), buf.len() as isize, 0x0800_0100) };
    // SAFETY : CopyName nous a donné la propriété de la chaîne.
    unsafe { CFRelease(s) };
    if !ok { return String::new(); }
    let end = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

/// Les instruments et effets AU installés, triés par nom.
pub fn list() -> Vec<AuInfo> {
    let mut out = Vec::new();
    for kind in [INSTRUMENT, EFFECT, MUSIC_EFFECT] {
        let want = Desc { kind, ..Default::default() };
        let mut c: AudioComponent = std::ptr::null_mut();
        loop {
            // SAFETY : description valide, `c` vient de l'appel précédent ou est nul.
            c = unsafe { AudioComponentFindNext(c, &want) };
            if c.is_null() { break; }
            let mut name: CFStringRef = std::ptr::null();
            let mut d = Desc::default();
            // SAFETY : `c` est un composant valide ; les sorties pointent sur nos variables.
            let (a, b) = unsafe { (AudioComponentCopyName(c, &mut name), AudioComponentGetDescription(c, &mut d)) };
            if a != 0 || b != 0 { continue; }
            out.push(AuInfo { name: cf_string(name), instrument: kind == INSTRUMENT, desc: (d.kind, d.sub, d.manu) });
        }
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

/// Le premier AU dont le nom contient `query` (sans tenir compte de la casse).
pub fn find(query: &str) -> Option<AuInfo> {
    let q = query.to_lowercase();
    let all = list();
    all.iter().find(|a| a.name.to_lowercase() == q).or_else(|| all.iter().find(|a| a.name.to_lowercase().contains(&q))).cloned()
}

/// Un instrument AU ouvert, prêt à jouer : notes MIDI dedans, stéréo flottant dehors.
pub struct AuInstrument {
    unit: AudioUnit,
    rate: f64,
    clock: f64,
    left: Vec<f32>,
    right: Vec<f32>,
}

// L'AU n'est touché que par le fil qui possède l'AuInstrument.
unsafe impl Send for AuInstrument {}

impl AuInstrument {
    pub fn open(info: &AuInfo, sample_rate: f64) -> Result<AuInstrument, String> {
        if !info.instrument { return Err(format!("{} est un effet, pas un instrument", info.name)); }
        let d = Desc { kind: info.desc.0, sub: info.desc.1, manu: info.desc.2, ..Default::default() };
        // SAFETY : description tirée de list() ; FindNext rend un composant ou nul.
        let c = unsafe { AudioComponentFindNext(std::ptr::null_mut(), &d) };
        if c.is_null() { return Err(format!("{} introuvable", info.name)); }
        let mut unit: AudioUnit = std::ptr::null_mut();
        // SAFETY : composant valide, sortie vers notre variable.
        let st = unsafe { AudioComponentInstanceNew(c, &mut unit) };
        if st != 0 || unit.is_null() { return Err(format!("{} ne s'ouvre pas (erreur {st})", info.name)); }
        let me = AuInstrument { unit, rate: sample_rate, clock: 0.0, left: vec![0.0; SLICE], right: vec![0.0; SLICE] };
        let fmt = Asbd {
            rate: sample_rate,
            format: fourcc(b"lpcm"),
            // flottant, rangé, non entrelacé
            flags: 1 | (1 << 3) | (1 << 5),
            bytes_per_packet: 4,
            frames_per_packet: 1,
            bytes_per_frame: 4,
            channels: 2,
            bits: 32,
            reserved: 0,
        };
        let max = SLICE as u32;
        // SAFETY : l'unité est ouverte ; les données pointent sur des valeurs de la bonne taille.
        unsafe {
            let st = AudioUnitSetProperty(unit, PROP_STREAM_FORMAT, SCOPE_OUTPUT, 0, &fmt as *const Asbd as *const c_void, std::mem::size_of::<Asbd>() as u32);
            if st != 0 { return Err(format!("{} refuse le format stéréo flottant (erreur {st})", info.name)); }
            AudioUnitSetProperty(unit, PROP_MAX_FRAMES, SCOPE_GLOBAL, 0, &max as *const u32 as *const c_void, 4);
            let st = AudioUnitInitialize(unit);
            if st != 0 { return Err(format!("{} ne s'initialise pas (erreur {st})", info.name)); }
        }
        Ok(me)
    }

    /// Un message MIDI (`status`, `d1`, `d2`) au début du prochain bloc.
    pub fn midi(&mut self, status: u8, d1: u8, d2: u8) {
        // SAFETY : unité initialisée.
        unsafe { MusicDeviceMIDIEvent(self.unit, status as u32, d1 as u32, d2 as u32, 0) };
    }

    pub fn note_on(&mut self, ch: u8, note: u8, vel: u8) { self.midi(0x90 | (ch & 15), note & 127, vel.clamp(1, 127)); }
    pub fn note_off(&mut self, ch: u8, note: u8) { self.midi(0x80 | (ch & 15), note & 127, 0); }

    /// Ajoute `frames` trames rendues par l'AU à `left` / `right`.
    pub fn render(&mut self, frames: usize, left: &mut Vec<f32>, right: &mut Vec<f32>) -> Result<(), String> {
        let mut done = 0;
        while done < frames {
            let n = (frames - done).min(SLICE);
            let mut list = BufferList2 {
                count: 2,
                buffers: [
                    Buffer { channels: 1, bytes: (n * 4) as u32, data: self.left.as_mut_ptr() as *mut c_void },
                    Buffer { channels: 1, bytes: (n * 4) as u32, data: self.right.as_mut_ptr() as *mut c_void },
                ],
            };
            let ts = TimeStamp { sample_time: self.clock, flags: 1, ..Default::default() };
            let mut flags = 0u32;
            // SAFETY : tampons de SLICE trames au moins, n <= SLICE, unité initialisée.
            let st = unsafe { AudioUnitRender(self.unit, &mut flags, &ts, 0, n as u32, &mut list) };
            if st != 0 { return Err(format!("rendu AU : erreur {st}")); }
            left.extend_from_slice(&self.left[..n]);
            right.extend_from_slice(&self.right[..n]);
            self.clock += n as f64;
            done += n;
        }
        Ok(())
    }

    pub fn sample_rate(&self) -> f64 { self.rate }
}

impl Drop for AuInstrument {
    fn drop(&mut self) {
        // SAFETY : l'unité vient d'AudioComponentInstanceNew et n'est plus utilisée.
        unsafe {
            AudioUnitUninitialize(self.unit);
            AudioComponentInstanceDispose(self.unit);
        }
    }
}

/// Une note à jouer : départ et fin en secondes, hauteur MIDI, vélocité 1..127.
#[derive(Debug, Clone, Copy)]
pub struct AuNote {
    pub on: f64,
    pub off: f64,
    pub midi: u8,
    pub vel: u8,
}

/// Joue `notes` dans l'instrument et rend `seconds` de stéréo (plus `tail` pour les queues).
pub fn render_notes(inst: &mut AuInstrument, notes: &[AuNote], seconds: f64) -> Result<(Vec<f32>, Vec<f32>), String> {
    // les événements dans l'ordre, chacun calé au bloc de 64 trames près
    let mut ev: Vec<(usize, bool, u8, u8)> = Vec::new();
    let sr = inst.sample_rate();
    for n in notes {
        if !(n.on.is_finite() && n.off.is_finite()) || n.on < 0.0 { continue; }
        ev.push(((n.on * sr) as usize, true, n.midi, n.vel));
        ev.push(((n.off.max(n.on + 0.005) * sr) as usize, false, n.midi, 0));
    }
    // à la même trame, les fins avant les débuts (une note répétée se relance)
    ev.sort_by_key(|e| (e.0, e.1));
    let total = (seconds.max(0.0) * sr) as usize;
    let (mut l, mut r) = (Vec::with_capacity(total), Vec::with_capacity(total));
    let mut pos = 0usize;
    for (at, on, m, v) in ev {
        let at = at.min(total);
        if at > pos { inst.render(at - pos, &mut l, &mut r)?; pos = at; }
        if on { inst.note_on(0, m, v) } else { inst.note_off(0, m) }
    }
    if total > pos { inst.render(total - pos, &mut l, &mut r)?; }
    Ok((l, r))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liste_les_au_d_apple() {
        let all = list();
        // le synthé General MIDI d'Apple est sur tous les Mac
        assert!(all.iter().any(|a| a.instrument && a.name.contains("DLSMusicDevice")), "{:?}", all.iter().map(|a| &a.name).collect::<Vec<_>>());
        assert!(all.iter().any(|a| !a.instrument), "aucun effet AU ?");
        assert!(find("dlsmusicdevice").is_some());
        assert!(find("pas un plug-in du tout").is_none());
    }

    #[test]
    fn joue_un_accord_dans_le_synthe_d_apple() {
        let info = find("DLSMusicDevice").unwrap();
        let mut inst = AuInstrument::open(&info, 48000.0).unwrap();
        let notes = [60u8, 64, 67].map(|m| AuNote { on: 0.1, off: 0.6, midi: m, vel: 100 });
        let (l, r) = render_notes(&mut inst, &notes, 1.0).unwrap();
        assert_eq!((l.len(), r.len()), (48000, 48000));
        let rms = |s: &[f32]| (s.iter().map(|x| x * x).sum::<f32>() / s.len().max(1) as f32).sqrt();
        // silence avant la note, du son pendant
        assert!(rms(&l[..4000]) < 1e-4, "{}", rms(&l[..4000]));
        assert!(rms(&l[6000..24000]) > 1e-3, "{}", rms(&l[6000..24000]));
        assert!(l.iter().chain(&r).all(|x| x.is_finite()));
        // un effet ne s'ouvre pas comme instrument
        if let Some(fx) = list().into_iter().find(|a| !a.instrument) {
            assert!(AuInstrument::open(&fx, 48000.0).is_err());
        }
    }
}
