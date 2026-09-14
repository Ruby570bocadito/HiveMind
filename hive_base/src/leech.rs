//! Leech — credential discovery & harvest EMULATION (Hive Colony red-team lab).
//!
//! Ronda 4 (emulación): la recolección real de credenciales (lectura de
//! `/etc/shadow`, scraping de memoria de procesos, LSASS, tokens de nube,
//! almacenes de navegador, etc.) fue ELIMINADA. El módulo conserva:
//!
//! - Los tipos (`LeechHarvest`, `CredType`) que consumen `comms` y la telemetría,
//!   para que el flujo de consenso y de mensajes no cambie.
//! - `discover_credential_sources()`: enumeración de solo lectura de RUTAS donde
//!   podrían existir credenciales (existencia de ficheros, sin leer contenido).
//! - `harvest_all()` / `harvest_high_value()`: devuelven hallazgos SIMULADOS con
//!   datos enmascarados, suficientes para probar pipelines de detección y
//!   priorización (`priority` 1-10) sin tocar ningún secreto real.
use tracing::info;

// ── types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LeechHarvest {
    pub credential_type: CredType,
    pub username: String,
    pub domain: String,
    pub data: String,
    pub source_process: String,
    pub priority: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CredType {
    ShadowHash,
    ProcessMemory,
    SSHKey,
    CloudTokenAWS,
    CloudTokenGCP,
    CloudTokenAzure,
    CloudTokenK8s,
    KerberosTGT,
    KerberosTGS,
    NTLMHash,
    ClearTextPassword,
    AccessToken,
    RDPCredential,
    VaultToken,
    GnupgKey,
    BrowserPassword,
}

impl CredType {
    /// Etiqueta corta usada por la telemetría (asset tags en `comms`).
    pub fn as_asset_tag(&self) -> &'static str {
        match self {
            CredType::ShadowHash => "shadow",
            CredType::ProcessMemory => "proc_mem",
            CredType::SSHKey => "ssh_key",
            CredType::CloudTokenAWS => "cloud_aws",
            CredType::CloudTokenGCP => "cloud_gcp",
            CredType::CloudTokenAzure => "cloud_azure",
            CredType::CloudTokenK8s => "cloud_k8s",
            CredType::KerberosTGT => "krb_tgt",
            CredType::KerberosTGS => "krb_tgs",
            CredType::NTLMHash => "ntlm",
            CredType::ClearTextPassword => "cleartext",
            CredType::AccessToken => "access_token",
            CredType::RDPCredential => "rdp",
            CredType::VaultToken => "vault",
            CredType::GnupgKey => "gnupg",
            CredType::BrowserPassword => "browser",
        }
    }
}

// ── enumeración de fuentes (solo lectura: existencia, nunca contenido) ──────

/// Rutas típicas de credenciales. La enumeración comprueba EXISTENCIA y
/// permisos, nunca contenido. Valor de entrenamiento: el defensor ve qué
/// fuentes de credenciales son localizables desde un agente comprometido.
const KNOWN_SOURCES: &[(&str, &str)] = &[
    ("/etc/shadow", "shadow"),
    ("/etc/sudoers", "sudoers"),
    ("/root/.ssh", "ssh_root_dir"),
    ("~/.ssh", "ssh_user_dir"),
    ("~/.aws/credentials", "aws_cli"),
    ("~/.config/gcloud", "gcloud_cli"),
    ("~/.kube/config", "kubeconfig"),
    ("~/.vault-token", "vault_token"),
    ("~/.gnupg", "gnupg"),
];

/// Enumera qué fuentes de credenciales EXISTEN en el host (sin leer contenido).
pub fn discover_credential_sources() -> Vec<String> {
    let mut found = Vec::new();
    for (path, tag) in KNOWN_SOURCES {
        let expanded = if path.starts_with('~') {
            match std::env::var("HOME") {
                Ok(home) => path.replacen('~', &home, 1),
                Err(_) => continue,
            }
        } else {
            (*path).to_string()
        };
        if std::path::Path::new(&expanded).exists() {
            found.push(tag.to_string());
        }
    }
    info!(
        "LEECH (recon): {} fuente(s) de credenciales localizadas [{}] — contenido NO leído (emulation mode)",
        found.len(),
        found.join(", ")
    );
    found
}

// ── recolección simulada ─────────────────────────────────────────────────────

fn simulated_harvest(types: &[CredType]) -> Vec<LeechHarvest> {
    types
        .iter()
        .map(|t| LeechHarvest {
            credential_type: t.clone(),
            username: "simulated_user".into(),
            domain: "simulated.lab".into(),
            // Dato enmascarado: nunca procede del host real.
            data: "SIMULATED::********".into(),
            source_process: "hive_emulation".into(),
            priority: match t {
                CredType::KerberosTGT | CredType::NTLMHash | CredType::ClearTextPassword => 10,
                CredType::ShadowHash | CredType::SSHKey => 8,
                CredType::CloudTokenAWS | CredType::CloudTokenAzure | CredType::CloudTokenGCP | CredType::CloudTokenK8s | CredType::VaultToken => 7,
                _ => 4,
            },
        })
        .collect()
}

/// Devuelve un lote SIMULADO de credenciales cubriendo todas las categorías.
/// No lee ficheros, memoria ni procesos.
pub fn harvest_all() -> Vec<LeechHarvest> {
    let batch = simulated_harvest(&[
        CredType::ShadowHash,
        CredType::SSHKey,
        CredType::CloudTokenAWS,
        CredType::CloudTokenK8s,
        CredType::KerberosTGT,
        CredType::NTLMHash,
        CredType::BrowserPassword,
    ]);
    info!(
        "LEECH (simulated): {} credenciales simuladas generadas (datos enmascarados) — harvest real deshabilitado (emulation mode)",
        batch.len()
    );
    batch
}

/// Subconjunto de alta prioridad (priority >= 7), también simulado.
pub fn harvest_high_value() -> Vec<LeechHarvest> {
    let batch = simulated_harvest(&[
        CredType::KerberosTGT,
        CredType::NTLMHash,
        CredType::CloudTokenAWS,
    ]);
    info!(
        "LEECH (simulated): {} credenciales high-value simuladas — harvest real deshabilitado (emulation mode)",
        batch.len()
    );
    batch
}
