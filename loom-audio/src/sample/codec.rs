//! Les autres formats que WAV : AIFF / AIFC (lu ici, il est simple), FLAC et MP3
//! (par symphonia, en Rust pur). Le format se reconnaît aux premiers octets, pas à
//! l'extension, qui ne sert que d'indice à symphonia.

use super::SampleData;

/// Les extensions que la banque charge.
pub const EXTENSIONS: [&str; 6] = ["wav", "aif", "aiff", "aifc", "flac", "mp3"];

/// Décode un fichier audio, quel que soit son format.
pub fn decode_any(bytes: &[u8], ext: &str) -> Result<SampleData, String> {
    match bytes.get(..4) {
        Some(b"RIFF") => super::wav::decode(bytes),
        Some(b"FORM") => decode_aiff(bytes),
        _ => decode_compressed(bytes, ext),
    }
}

/// Le flottant étendu 80 bits d'AIFF (la fréquence d'échantillonnage).
fn extended(b: &[u8]) -> f64 {
    let exp = (((b[0] & 0x7f) as i32) << 8) | b[1] as i32;
    let mant = u64::from_be_bytes([b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9]]);
    if exp == 0 && mant == 0 { return 0.0; }
    let v = mant as f64 * 2f64.powi(exp - 16383 - 63);
    if b[0] & 0x80 != 0 { -v } else { v }
}

/// AIFF et AIFC (non compressé `NONE`, petit-boutiste `sowt`, flottant `fl32`).
pub fn decode_aiff(b: &[u8]) -> Result<SampleData, String> {
    if b.len() < 12 || &b[..4] != b"FORM" { return Err("pas un fichier AIFF".into()); }
    let aifc = match &b[8..12] { b"AIFF" => false, b"AIFC" => true, _ => return Err("pas un fichier AIFF".into()) };
    let (mut comm, mut ssnd) = (None, None);
    let mut i = 12;
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let size = u32::from_be_bytes([b[i + 4], b[i + 5], b[i + 6], b[i + 7]]) as usize;
        let body = &b[i + 8..(i + 8 + size).min(b.len())];
        match id { b"COMM" => comm = Some(body), b"SSND" => ssnd = Some(body), _ => {} }
        i += 8 + size + (size & 1);
    }
    let comm = comm.filter(|c| c.len() >= 18).ok_or("AIFF : bloc COMM absent")?;
    let ssnd = ssnd.filter(|s| s.len() >= 8).ok_or("AIFF : bloc SSND absent")?;
    let channels = u16::from_be_bytes([comm[0], comm[1]]) as usize;
    let frames = u32::from_be_bytes([comm[2], comm[3], comm[4], comm[5]]) as usize;
    let bits = u16::from_be_bytes([comm[6], comm[7]]) as usize;
    let rate = extended(&comm[8..18]);
    let kind: [u8; 4] = if aifc && comm.len() >= 22 { [comm[18], comm[19], comm[20], comm[21]] } else { *b"NONE" };
    let little = match &kind { b"NONE" | b"twos" => false, b"sowt" => true, b"fl32" | b"FL32" => false, _ => return Err(format!("AIFC : compression {} non prise en charge", String::from_utf8_lossy(&kind))) };
    let float = matches!(&kind, b"fl32" | b"FL32");
    if channels == 0 || !(1..=32).contains(&bits) || !rate.is_finite() || rate <= 0.0 || rate > 1e7 { return Err("AIFF : en-tête invalide".into()); }
    let offset = u32::from_be_bytes([ssnd[0], ssnd[1], ssnd[2], ssnd[3]]) as usize;
    let data = ssnd.get(8 + offset..).unwrap_or(&[]);
    let width = if float { 4 } else { bits.div_ceil(8) };
    let frames = frames.min(data.len() / (width * channels));
    let keep = channels.min(2);
    let mut out = vec![Vec::with_capacity(frames); keep];
    for f in 0..frames {
        for (c, ch) in out.iter_mut().enumerate() {
            let p = &data[(f * channels + c) * width..][..width];
            let v = if float {
                let w = [p[0], p[1], p[2], p[3]];
                f32::from_be_bytes(w)
            } else {
                // entier signé, ramené dans [-1, 1]
                let mut x: i64 = 0;
                for k in 0..width { x = (x << 8) | p[if little { width - 1 - k } else { k }] as i64; }
                let shift = 64 - 8 * width as u32;
                ((x << shift) >> shift) as f32 / (1u64 << (8 * width - 1)) as f32
            };
            ch.push(v);
        }
    }
    Ok(SampleData { sample_rate: rate as f32, channels: out })
}

/// FLAC et MP3 par symphonia.
fn decode_compressed(bytes: &[u8], ext: &str) -> Result<SampleData, String> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::errors::Error;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let mss = MediaSourceStream::new(Box::new(std::io::Cursor::new(bytes.to_vec())), Default::default());
    let mut hint = Hint::new();
    if !ext.is_empty() { hint.with_extension(ext); }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| format!("format audio inconnu : {e}"))?;
    let mut format = probed.format;
    let track = format.default_track().ok_or("aucune piste audio")?;
    let id = track.id;
    let mut rate = track.codec_params.sample_rate.unwrap_or(0) as f32;
    let mut dec = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("décodeur : {e}"))?;
    let mut out: Vec<Vec<f32>> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(Error::ResetRequired) => break,
            Err(e) => return Err(format!("lecture : {e}")),
        };
        if packet.track_id() != id { continue; }
        let buf = match dec.decode(&packet) {
            Ok(b) => b,
            Err(Error::DecodeError(_)) => continue, // une trame abîmée : on passe
            Err(e) => return Err(format!("décodage : {e}")),
        };
        let spec = *buf.spec();
        rate = spec.rate as f32;
        let n = spec.channels.count().max(1);
        let mut sb = SampleBuffer::<f32>::new(buf.capacity() as u64, spec);
        sb.copy_interleaved_ref(buf);
        if out.is_empty() { out = vec![Vec::new(); n.min(2)]; }
        for frame in sb.samples().chunks(n) {
            for (c, ch) in out.iter_mut().enumerate() { ch.push(frame[c.min(frame.len() - 1)]); }
        }
    }
    if out.is_empty() || !rate.is_finite() || rate <= 0.0 { return Err("fichier audio vide".into()); }
    Ok(SampleData { sample_rate: rate, channels: out })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un AIFF 16 bits mono à 44,1 kHz (le 80 bits de 44100 : 400E AC44 0000 0000 0000).
    fn aiff16(samples: &[i16]) -> Vec<u8> {
        let mut comm = Vec::new();
        comm.extend(1u16.to_be_bytes());
        comm.extend((samples.len() as u32).to_be_bytes());
        comm.extend(16u16.to_be_bytes());
        comm.extend([0x40, 0x0E, 0xAC, 0x44, 0, 0, 0, 0, 0, 0]);
        let mut ssnd = vec![0u8; 8];
        for s in samples { ssnd.extend(s.to_be_bytes()); }
        let mut body = b"AIFF".to_vec();
        body.extend(b"COMM"); body.extend((comm.len() as u32).to_be_bytes()); body.extend(&comm);
        body.extend(b"SSND"); body.extend((ssnd.len() as u32).to_be_bytes()); body.extend(&ssnd);
        let mut f = b"FORM".to_vec();
        f.extend((body.len() as u32).to_be_bytes());
        f.extend(body);
        f
    }

    #[test]
    fn lit_l_aiff() {
        let d = decode_any(&aiff16(&[0, 16384, -16384, 32767]), "aif").unwrap();
        assert_eq!(d.sample_rate, 44100.0);
        assert_eq!(d.channels.len(), 1);
        assert_eq!(d.channels[0][..3], [0.0, 0.5, -0.5]);
        assert!((d.channels[0][3] - 1.0).abs() < 1e-4);
        assert!(decode_any(b"FORMxxxxWAVE", "aif").is_err());
        assert!(decode_any(b"pas du son", "mp3").is_err());
    }

    /// Bout en bout sur macOS : `afconvert` fabrique un AIFF et un FLAC depuis un WAV,
    /// LOOM les relit et retrouve les mêmes échantillons (une seconde : sous un bloc FLAC,
    /// 4608 trames, afconvert écrit un en-tête vide). `cargo test -p loom-audio codec -- --ignored`
    #[test]
    #[ignore]
    fn relit_aiff_et_flac_de_afconvert() {
        let dir = std::env::temp_dir().join(format!("loom-codec-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let wav = dir.join("s.wav");
        let pcm: Vec<i16> = (0..44100).map(|i| ((i as f32 * 0.05).sin() * 12000.0) as i16).collect();
        let mut w = b"RIFF".to_vec();
        w.extend((36 + pcm.len() as u32 * 2).to_le_bytes());
        w.extend(b"WAVEfmt "); w.extend(16u32.to_le_bytes()); w.extend(1u16.to_le_bytes()); w.extend(1u16.to_le_bytes());
        w.extend(44100u32.to_le_bytes()); w.extend(88200u32.to_le_bytes()); w.extend(2u16.to_le_bytes()); w.extend(16u16.to_le_bytes());
        w.extend(b"data"); w.extend((pcm.len() as u32 * 2).to_le_bytes());
        for s in &pcm { w.extend(s.to_le_bytes()); }
        std::fs::write(&wav, &w).unwrap();
        let reference = decode_any(&w, "wav").unwrap();
        for (ext, args) in [("aif", ["-f", "AIFF", "-d", "BEI16"]), ("flac", ["-f", "flac", "-d", "flac"])] {
            let out = dir.join(format!("s.{ext}"));
            assert!(std::process::Command::new("afconvert").arg(&wav).args(args).arg(&out).status().unwrap().success());
            let raw = std::fs::read(&out).unwrap();
            let d = decode_any(&raw, ext).unwrap_or_else(|e| panic!("{ext} {:?} {e}", &raw[..raw.len().min(8)]));
            assert_eq!(d.sample_rate, 44100.0, "{ext}");
            let n = reference.frames().min(d.frames());
            assert!(n >= 44000, "{ext} {n}");
            let worst = (0..n).map(|i| (d.channels[0][i] - reference.channels[0][i]).abs()).fold(0.0, f32::max);
            assert!(worst < 1e-3, "{ext} {worst}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
