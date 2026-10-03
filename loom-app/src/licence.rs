//! La licence de LOOM : un essai de quelques jours, puis une clé achetée sur
//! Lemon Squeezy.
//!
//! - Au premier lancement, l'essai commence (`ESSAI_JOURS` jours). La date de
//!   départ est notée à deux endroits ; on garde la plus ancienne.
//! - `loom licence CLÉ` active la clé une fois en ligne (API Lemon Squeezy,
//!   par `curl`), puis LOOM marche hors ligne.
//! - Tous les 30 jours, une vérification en ligne se fait si le réseau répond.
//!   Une clé remboursée ou désactivée s'arrête ; sans réseau, rien ne change.
//! - `loom licence --retirer` libère la clé pour un autre ordinateur.
//!
//! Un essai hors ligne se contourne toujours (on peut effacer les fichiers) :
//! le but est d'être honnête avec les gens honnêtes, pas une forteresse.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

/// Durée de l'essai.
pub const ESSAI_JOURS: u64 = 14;
/// Où acheter une clé.
pub const ACHAT_URL: &str = "https://munaypassaqui.com/loom/";
/// Le produit LOOM chez Lemon Squeezy (0 = pas encore créé : toute clé valide du magasin passe).
pub const STORE_ID: u64 = 0;
pub const PRODUCT_ID: u64 = 0;
const API: &str = "https://api.lemonsqueezy.com/v1/licenses";
const JOUR: u64 = 86_400;
const REVERIFIER: u64 = 30 * JOUR;

/// Ce que LOOM a le droit de faire maintenant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statut {
    /// Une clé activée (les 4 derniers caractères, pour l'affichage).
    Licence(String),
    /// En essai, jours restants (au moins 1).
    Essai(u64),
    /// Essai fini, pas de clé.
    Fini,
}

/// L'état enregistré.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Etat {
    pub essai: Option<u64>,
    pub cle: Option<String>,
    pub instance: Option<String>,
    pub verifie: Option<u64>,
}

impl Etat {
    pub fn lire(texte: &str) -> Etat {
        let mut e = Etat::default();
        for l in texte.lines() {
            let Some((k, v)) = l.split_once(' ') else { continue };
            let v = v.trim();
            match k {
                "essai" => e.essai = v.parse().ok(),
                "cle" if !v.is_empty() => e.cle = Some(v.to_string()),
                "instance" if !v.is_empty() => e.instance = Some(v.to_string()),
                "verifie" => e.verifie = v.parse().ok(),
                _ => {}
            }
        }
        e
    }

    pub fn ecrire(&self) -> String {
        let mut s = String::new();
        if let Some(t) = self.essai { s += &format!("essai {t}\n"); }
        if let Some(c) = &self.cle { s += &format!("cle {c}\n"); }
        if let Some(i) = &self.instance { s += &format!("instance {i}\n"); }
        if let Some(t) = self.verifie { s += &format!("verifie {t}\n"); }
        s
    }

    /// Le statut à l'instant `now` (secondes Unix).
    pub fn statut(&self, now: u64) -> Statut {
        if let (Some(c), Some(_)) = (&self.cle, &self.instance) {
            return Statut::Licence(c.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect());
        }
        let debut = self.essai.unwrap_or(now).min(now);
        let passe = (now - debut) / JOUR;
        if passe < ESSAI_JOURS { Statut::Essai(ESSAI_JOURS - passe) } else { Statut::Fini }
    }
}

fn maintenant() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()) }

fn fichiers() -> (Option<PathBuf>, Option<PathBuf>) {
    (loom_audio::paths::state_dir().map(|d| d.join("licence")), loom_audio::paths::data_dir().map(|d| d.join(".essai")))
}

/// L'état enregistré, l'essai démarré s'il ne l'était pas.
pub fn charger() -> Etat {
    let (principal, double) = fichiers();
    let mut e = principal.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| Etat::lire(&t)).unwrap_or_default();
    if let Some(p) = principal.as_ref().filter(|p| p.is_file()) { proteger(p); }
    let autre = double.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| t.trim().parse::<u64>().ok());
    let debut = [e.essai, autre].into_iter().flatten().min().unwrap_or_else(maintenant);
    if e.essai != Some(debut) || autre != Some(debut) {
        e.essai = Some(debut);
        let _ = enregistrer(&e);
    }
    e
}

/// La clé : lisible par son propriétaire seulement.
fn proteger(p: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = p;
}

fn enregistrer(e: &Etat) -> std::io::Result<()> {
    let (principal, double) = fichiers();
    if let Some(p) = principal {
        if let Some(d) = p.parent() { std::fs::create_dir_all(d)?; }
        std::fs::write(&p, e.ecrire())?;
        proteger(&p);
    }
    if let (Some(p), Some(t)) = (double, e.essai) {
        if let Some(d) = p.parent() { std::fs::create_dir_all(d)?; }
        std::fs::write(p, format!("{t}\n"))?;
    }
    Ok(())
}

/// Le statut actuel (vérifie la clé en ligne tous les 30 jours, sans bloquer si le réseau manque).
pub fn statut() -> Statut {
    let mut e = charger();
    let now = maintenant();
    if let (Some(cle), Some(inst)) = (e.cle.clone(), e.instance.clone()) {
        if now.saturating_sub(e.verifie.unwrap_or(0)) > REVERIFIER {
            match appeler("validate", &[("license_key", &cle), ("instance_id", &inst)]) {
                Ok(json) if json_bool(&json, "valid") == Some(false) => {
                    e.cle = None;
                    e.instance = None;
                }
                Ok(_) => e.verifie = Some(now),
                Err(_) => {} // pas de réseau : on réessaiera
            }
            let _ = enregistrer(&e);
        }
    }
    e.statut(now)
}

/// La phrase à montrer quand l'essai est fini.
pub fn message_fini() -> String {
    format!("L'essai de LOOM ({ESSAI_JOURS} jours) est terminé. Une clé s'achète sur {ACHAT_URL}\npuis : loom licence VOTRE-CLÉ")
}

/// `Ok(Some(jours))` en essai, `Ok(None)` avec une clé, `Err(message)` quand c'est fini.
pub fn controler() -> Result<Option<u64>, String> {
    match statut() {
        Statut::Licence(_) => Ok(None),
        Statut::Essai(j) => Ok(Some(j)),
        Statut::Fini => Err(message_fini()),
    }
}

/// Active `cle` sur cet ordinateur.
pub fn activer(cle: &str) -> Result<String, String> {
    let cle = cle.trim();
    if cle.len() < 8 || cle.len() > 128 || !cle.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err("cette clé n'a pas la bonne forme (lettres, chiffres et tirets)".into());
    }
    let nom = std::env::var("HOSTNAME").or_else(|_| std::env::var("COMPUTERNAME")).unwrap_or_else(|_| "ordinateur".into());
    let json = appeler("activate", &[("license_key", cle), ("instance_name", &format!("LOOM sur {nom}"))])?;
    if json_bool(&json, "activated") != Some(true) {
        return Err(json_str(&json, "error").unwrap_or_else(|| "clé refusée".into()));
    }
    verifier_produit(&json)?;
    let inst = json_str_in(&json, "\"instance\"", "id").ok_or("réponse sans identifiant d'activation")?;
    let mut e = charger();
    e.cle = Some(cle.to_string());
    e.instance = Some(inst);
    e.verifie = Some(maintenant());
    enregistrer(&e).map_err(|e| e.to_string())?;
    Ok(format!("LOOM est activé. Merci ! ({})", match e.statut(maintenant()) { Statut::Licence(f) => format!("clé …{f}"), _ => String::new() }))
}

/// Libère la clé de cet ordinateur.
pub fn retirer() -> Result<String, String> {
    let mut e = charger();
    let (Some(cle), Some(inst)) = (e.cle.clone(), e.instance.clone()) else { return Err("pas de clé sur cet ordinateur".into()) };
    let json = appeler("deactivate", &[("license_key", &cle), ("instance_id", &inst)])?;
    if json_bool(&json, "deactivated") != Some(true) {
        return Err(json_str(&json, "error").unwrap_or_else(|| "désactivation refusée".into()));
    }
    e.cle = None;
    e.instance = None;
    enregistrer(&e).map_err(|e| e.to_string())?;
    Ok("Clé retirée de cet ordinateur : elle peut servir ailleurs.".into())
}

fn verifier_produit(json: &str) -> Result<(), String> {
    let ok = |champ: &str, voulu: u64| voulu == 0 || json_num(json, champ) == Some(voulu);
    if ok("store_id", STORE_ID) && ok("product_id", PRODUCT_ID) { Ok(()) } else { Err("cette clé n'est pas une clé LOOM".into()) }
}

/// POST vers l'API de licences, par `curl` (présent sur macOS, Windows 10+ et Linux).
fn appeler(action: &str, champs: &[(&str, &str)]) -> Result<String, String> {
    let corps: String = champs.iter().map(|(k, v)| format!("{k}={}", encoder(v))).collect::<Vec<_>>().join("&");
    let mut enfant = Command::new(if cfg!(windows) { "curl.exe" } else { "curl" })
        .args(["-sS", "--max-time", "15", "-X", "POST", "-H", "Accept: application/json", "--data-binary", "@-", &format!("{API}/{action}")])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().map_err(|_| "curl introuvable : impossible de joindre le serveur de licences".to_string())?;
    {
        use std::io::Write;
        // la clé passe par l'entrée standard, jamais dans la ligne de commande
        let mut stdin = enfant.stdin.take().ok_or("curl : pas d'entrée")?;
        stdin.write_all(corps.as_bytes()).map_err(|e| e.to_string())?;
    }
    let out = enfant.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() { return Err(format!("réseau : {}", String::from_utf8_lossy(&out.stderr).trim())); }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn encoder(s: &str) -> String {
    s.bytes().map(|b| match b {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
        b' ' => "+".into(),
        _ => format!("%{b:02X}"),
    }).collect()
}

// ---- lecture minimale du JSON de Lemon Squeezy (pas de dépendance) ----

fn apres<'a>(json: &'a str, champ: &str) -> Option<&'a str> {
    let k = format!("\"{champ}\"");
    let i = json.find(&k)? + k.len();
    let rest = json[i..].trim_start().strip_prefix(':')?;
    Some(rest.trim_start())
}

pub fn json_bool(json: &str, champ: &str) -> Option<bool> {
    let r = apres(json, champ)?;
    if r.starts_with("true") { Some(true) } else if r.starts_with("false") { Some(false) } else { None }
}

pub fn json_num(json: &str, champ: &str) -> Option<u64> {
    let r = apres(json, champ)?;
    let n: String = r.chars().take_while(|c| c.is_ascii_digit()).collect();
    n.parse().ok()
}

pub fn json_str(json: &str, champ: &str) -> Option<String> {
    let r = apres(json, champ)?.strip_prefix('"')?;
    let mut out = String::new();
    let mut it = r.chars();
    while let Some(c) = it.next() {
        match c {
            '"' => return Some(out),
            '\\' => { if let Some(n) = it.next() { out.push(n); } }
            c => out.push(c),
        }
    }
    None
}

/// `champ` à l'intérieur de l'objet qui suit `objet` (ex. `"instance": {"id": …}`).
pub fn json_str_in(json: &str, objet: &str, champ: &str) -> Option<String> {
    let i = json.find(objet)?;
    json_str(&json[i..], champ)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn essai_puis_fin() {
        let t0 = 1_800_000_000;
        let e = Etat { essai: Some(t0), ..Default::default() };
        assert_eq!(e.statut(t0), Statut::Essai(ESSAI_JOURS));
        assert_eq!(e.statut(t0 + 3 * JOUR + 5), Statut::Essai(ESSAI_JOURS - 3));
        assert_eq!(e.statut(t0 + ESSAI_JOURS * JOUR), Statut::Fini);
        // une horloge reculée ne rallonge pas l'essai au-delà du début
        assert_eq!(e.statut(t0 - 100 * JOUR), Statut::Essai(ESSAI_JOURS));
        let l = Etat { cle: Some("ABCD-1234-WXYZ".into()), instance: Some("i".into()), ..e.clone() };
        assert_eq!(l.statut(t0 + 999 * JOUR), Statut::Licence("WXYZ".into()));
        assert_eq!(Etat::lire(&l.ecrire()), l);
    }

    #[test]
    fn reponses_lemon_squeezy() {
        let ok = r#"{"activated":true,"error":null,"license_key":{"id":1,"status":"active","key":"38b1460a-5104-4067-a91d-77b872934d51"},"instance":{"id":"47596ad9-a811-4ebf-ac8a-03fc7b6d2a17","name":"Test"},"meta":{"store_id":1234,"product_id":5678}}"#;
        assert_eq!(json_bool(ok, "activated"), Some(true));
        assert_eq!(json_str_in(ok, "\"instance\"", "id").as_deref(), Some("47596ad9-a811-4ebf-ac8a-03fc7b6d2a17"));
        assert_eq!(json_num(ok, "product_id"), Some(5678));
        let non = r#"{"activated":false,"error":"This license key has reached the activation limit.","license_key":{"id":1}}"#;
        assert_eq!(json_bool(non, "activated"), Some(false));
        assert_eq!(json_str(non, "error").as_deref(), Some("This license key has reached the activation limit."));
        assert_eq!(json_bool(r#"{"valid": false}"#, "valid"), Some(false));
        assert_eq!(encoder("LOOM sur mac&co"), "LOOM+sur+mac%26co");
        assert!(activer("pas une clé !").is_err());
    }
}
