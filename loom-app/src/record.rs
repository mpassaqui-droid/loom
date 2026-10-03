//! Enregistrer au micro : une prise calée sur les barres, rangée dans « mes sons »,
//! jouée par une ligne `take1: x | sample take1 | stretch | loopAt N` qui suit le
//! tempo. Enregistrer de nouveau sur la même prise la superpose (overdub).
//!
//! Ici la partie sans micro ni carte son : couper au bon endroit, mélanger,
//! écrire le fichier. Le micro lui-même est `loom_audio::input::MicInput`.

use std::path::PathBuf;

pub use loom_core::capture::{layout_take, take_line, take_window};

/// Superpose `new` à `old` (la plus longue des deux fixe la durée), sans dépasser ±1.
pub fn overdub(old: &[f32], new: &[f32]) -> Vec<f32> {
    (0..old.len().max(new.len()))
        .map(|i| (old.get(i).copied().unwrap_or(0.0) + new.get(i).copied().unwrap_or(0.0)).clamp(-1.0, 1.0))
        .collect()
}

/// Le premier nom libre parmi `take1`, `take2`…
pub fn next_take_name(existing: &[String]) -> String {
    (1..).map(|i| format!("take{i}")).find(|n| !existing.contains(n)).unwrap()
}

/// Écrit la prise dans « mes sons » (`NOM.wav`, stéréo flottant), rangée mesure par
/// mesure (voir [`layout_take`]). Avec `overdub`, la prise du même nom reste dessous ;
/// si elle est illisible ou à une autre fréquence, rien n'est écrit (on ne l'écrase
/// jamais). Rend le chemin et le nombre de mesures de la boucle.
pub fn save_take(name: &str, mono: &[f32], bars: u32, first_bar: i64, sample_rate: u32, overdub_on: bool) -> Result<(PathBuf, u32), String> {
    let dir = crate::samples::user_dir().ok_or("pas de dossier personnel")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{name}.wav"));
    let old_mono = if overdub_on && path.is_file() {
        let old = loom_audio::sample::SampleData::from_file(&path).map_err(|e| format!("{name}.wav illisible, gardé tel quel : {e}"))?;
        if old.sample_rate as u32 != sample_rate {
            return Err(format!("{name}.wav est à {} Hz, le micro à {sample_rate} Hz : prise gardée telle quelle", old.sample_rate));
        }
        Some((0..old.frames()).map(|i| old.channels.iter().map(|c| c[i]).sum::<f32>() / old.channels.len() as f32).collect::<Vec<f32>>())
    } else { None };
    let (data, loop_bars) = layout_take(mono, bars, first_bar, old_mono.as_deref());
    let stereo: Vec<f32> = data.iter().flat_map(|s| [*s, *s]).collect();
    loom_audio::wavout::write_wav_as(&path, &stereo, sample_rate, loom_audio::wavout::WavFormat::Float32)
        .map_err(|e| e.to_string())?;
    // la banque relit le dossier : la prise (ou sa nouvelle couche) s'entend tout de suite
    loom_audio::reload_bank();
    Ok((path, loop_bars))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coupe_sur_la_barre() {
        // 1 000 échantillons par seconde, mesure de 2 s ; le micro démarre à la mesure 3,5
        let s: Vec<f32> = (0..9000).map(|i| i as f32).collect();
        let (w, bars, first) = take_window(&s, 1000, 3.5, 2.0, 0.0).unwrap();
        // la barre 4 tombe 1 s plus tard : échantillon 1000 ; reste 8000 → 4 mesures complètes → 2^k = 4
        assert_eq!((w[0], bars, w.len(), first), (1000.0, 4, 8000, 4));
        // un départ juste après la barre compte pour elle, et la latence recule la coupe
        let (w, bars, first) = take_window(&s, 1000, 3.02, 2.0, 0.01).unwrap();
        assert_eq!((w[0], bars, first), (0.0, 4, 3));
        assert!(take_window(&s[..1500], 1000, 3.5, 2.0, 0.0).is_none());
    }

    #[test]
    fn superpose_et_nomme() {
        assert_eq!(overdub(&[0.5, 0.8], &[0.25, 0.5, 0.1]), vec![0.75, 1.0, 0.1]);
        assert_eq!(next_take_name(&["take1".into(), "take3".into()]), "take2");
        let l = take_line("take2", 4);
        assert_eq!(l, "take2: x | sample take2 | stretch | loopAt 4 | slow 4");
        let prog = loom_core::Program::parse(&l);
        assert!(prog.voices[0].synth.stretch && prog.voices[0].synth.loop_at == Some(4.0));
        // un seul départ toutes les 4 mesures, sur les mesures 0, 4, 8…
        let hits: Vec<i64> = (0..8).filter(|b| !prog.schedule_bar(*b).is_empty()).collect();
        assert_eq!(hits, [0, 4]);
    }

    #[test]
    fn range_la_prise_mesure_par_mesure() {
        // 4 échantillons par mesure ; la prise de 2 mesures commence à la mesure 3 (impaire)
        let new = [1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0];
        let (out, l) = layout_take(&new, 2, 3, None);
        // la place 0 (mesures paires) reçoit la 2e mesure jouée (mesure 4), la place 1 la 1re
        assert_eq!((l, &out[..]), (2, &[2.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0][..]));
        // superposée à une boucle d'une mesure (0,1) : l'ancienne se répète dessous
        let quiet: Vec<f32> = new.iter().map(|x| x * 0.25).collect();
        let old = [0.1, 0.1, 0.1, 0.1];
        let (out, l) = layout_take(&quiet, 2, 3, Some(&old));
        assert_eq!(l, 2);
        assert!((out[0] - 0.6).abs() < 1e-6 && (out[4] - 0.35).abs() < 1e-6, "{out:?}");
        // une prise d'une mesure sur une boucle de 4 : elle se répète sur les 4 places
        let old4 = [0.0; 16];
        let (out, l) = layout_take(&[0.5; 4], 1, 7, Some(&old4));
        assert_eq!((l, out.len(), out[15]), (4, 16, 0.5));
    }
}
