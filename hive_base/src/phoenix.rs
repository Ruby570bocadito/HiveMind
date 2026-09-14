//! Phoenix — colony genome modeling + persistence EMULATION.
//! Hive Colony (red-team lab edition).
//!
//! Ronda 4 (emulación): las escrituras de persistencia real (fragmentos en
//! SPI/MBR/UEFI/bloques defectuosos, mecanismos systemd/cron, chattr +i,
//! reconstrucción de binarios en disco) fueron ELIMINADAS. Se conserva:
//!
//! - El modelado COMPLETO del genoma de la colonia (`ColonyGenome`,
//!   `AgentBlueprint`, `GenomeFragment`, `FragmentLocation`) y sus funciones
//!   puras en memoria (`generate_genome`, `fragment_genome`,
//!   `reassemble_genome`, `self_heal`) — es el valor arquitectónico del
//!   módulo y permite probar recuperación topológica sin tocar disco.
//! - `hide` / `hide_fragment` / `install_persistence` / `rebuild_from_genome`
//!   → SIMULADOS: registran el paso y devuelven resultados coherentes sin
//!   escribir nada en el sistema.
//! - `scan_for_fragments`: lectura de directorio (solo lectura).
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::info;
use uuid::Uuid;

// ── tipos del genoma ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColonyGenome {
    pub genome_id: Uuid,
    pub agent_blueprints: Vec<AgentBlueprint>,
    pub config_snapshot: HashMap<String, String>,
    pub timestamp: u64,
    pub compression: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentBlueprint {
    pub role: String,
    pub binary_hash: String,
    pub binary_size: u64,
    pub policy: HashMap<String, String>,
    pub encrypted_chunk: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenomeFragment {
    pub fragment_id: u32,
    pub total_fragments: u32,
    pub genome_id: Uuid,
    pub data: Vec<u8>,
    pub location: FragmentLocation,
    pub stored_path: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum FragmentLocation {
    SpiFlash,
    BadBlocks,
    MbrGpt,
    UefiVariable,
    HostProtectedArea,
}

impl FragmentLocation {
    /// Descripción documental de la técnica que *representa* cada ubicación.
    pub fn describe(&self) -> &'static str {
        match self {
            FragmentLocation::SpiFlash => "SPI flash area (simulado)",
            FragmentLocation::BadBlocks => "bad block area (simulado)",
            FragmentLocation::MbrGpt => "MBR/GPT reserved (simulado)",
            FragmentLocation::UefiVariable => "UEFI variable (simulado)",
            FragmentLocation::HostProtectedArea => "host protected area (simulado)",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceMechanism {
    pub name: String,
    pub path: String,
    pub mechanism_type: String,
    pub installed: bool,
    pub description: String,
}

// ── Phoenix ──────────────────────────────────────────────────────────────────

pub struct Phoenix;

impl Default for Phoenix {
    fn default() -> Self {
        Self::new()
    }
}

impl Phoenix {
    pub fn new() -> Self {
        Self
    }

    /// Genera el genoma de la colonia a partir de blueprints (en memoria).
    pub fn generate_genome(blueprints: Vec<AgentBlueprint>) -> ColonyGenome {
        let mut config = HashMap::new();
        config.insert("heartbeat_interval".into(), "10".into());
        config.insert("consensus_threshold".into(), "0.66".into());
        config.insert("max_hops".into(), "10".into());
        config.insert("safe_mode".into(), "true".into());
        config.insert("emulation".into(), "true".into());

        ColonyGenome {
            genome_id: Uuid::new_v4(),
            agent_blueprints: blueprints,
            config_snapshot: config,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            compression: "none".into(),
        }
    }

    /// Fragmenta el genoma en N piezas (en memoria) asignando ubicaciones
    /// documentales — no escribe nada.
    pub fn fragment_genome(genome: &ColonyGenome, num_fragments: u32) -> Vec<GenomeFragment> {
        let data = serde_json::to_vec(genome).unwrap_or_default();
        let chunk_size = (data.len() as f32 / num_fragments as f32).ceil() as usize;
        let mut fragments = Vec::new();

        for i in 0..num_fragments {
            let start = (i as usize) * chunk_size;
            let end = (start + chunk_size).min(data.len());
            let chunk = if start < data.len() {
                data[start..end].to_vec()
            } else {
                vec![]
            };

            let location = match i % 4 {
                0 => FragmentLocation::SpiFlash,
                1 => FragmentLocation::BadBlocks,
                2 => FragmentLocation::MbrGpt,
                _ => FragmentLocation::UefiVariable,
            };

            fragments.push(GenomeFragment {
                fragment_id: i,
                total_fragments: num_fragments,
                genome_id: genome.genome_id,
                data: chunk,
                location,
                stored_path: None,
            });
        }

        fragments
    }

    /// SIMULADO (ronda 4): no escribe el fragmento en ninguna ubicación.
    /// Devuelve una descripción documental de dónde *se habría* escondido.
    pub fn hide_fragment(fragment: &GenomeFragment, base_path: &Path) -> Result<String, String> {
        info!(
            "PHOENIX (simulated): fragmento {} -> {} en base '{}' — sin escritura (emulation mode)",
            fragment.fragment_id,
            fragment.location.describe(),
            base_path.display()
        );
        Ok(format!(
            "simulated: fragment {} would be hidden in {} (no data written; emulation mode)",
            fragment.fragment_id,
            fragment.location.describe()
        ))
    }

    /// SIMULADO (ronda 4): marca la ubicación documental en el fragmento pero
    /// no cifra ni escribe nada.
    pub fn hide(fragment: &mut GenomeFragment, base_path: &Path) -> Result<String, String> {
        let msg = Self::hide_fragment(fragment, base_path)?;
        fragment.stored_path = Some(format!(
            "{}/(simulado::{})",
            base_path.display(),
            format!("{:?}", fragment.location).to_lowercase()
        ));
        Ok(msg)
    }

    /// SIMULADO (ronda 4): los fragmentos ya no se cifran al esconderse, así
    /// que la recuperación devuelve los datos tal cual (operación en memoria).
    pub fn recover(fragment: &GenomeFragment) -> Result<Vec<u8>, String> {
        Ok(fragment.data.clone())
    }

    /// SIMULADO (ronda 4): no instala ningún mecanismo de persistencia.
    /// Devuelve los mecanismos *que se habrían instalado*, con `installed: false`.
    pub fn install_persistence(loader_script: &str, base_path: &Path) -> Vec<PersistenceMechanism> {
        info!(
            "PHOENIX (simulated): install_persistence (loader {}B, base '{}') — nada instalado (emulation mode)",
            loader_script.len(),
            base_path.display()
        );
        vec![
            PersistenceMechanism {
                name: "systemd-user-unit (simulated)".into(),
                path: "~/.config/systemd/user/hive.service".into(),
                mechanism_type: "service".into(),
                installed: false,
                description: "simulated: persistence disabled (emulation mode, ronda 4)".into(),
            },
            PersistenceMechanism {
                name: "cron-entry (simulated)".into(),
                path: "/etc/cron.d/hive".into(),
                mechanism_type: "scheduler".into(),
                installed: false,
                description: "simulated: persistence disabled (emulation mode, ronda 4)".into(),
            },
        ]
    }

    /// Recupera el genoma a partir de fragmentos completos (en memoria).
    /// Función pura: reconstrucción por orden de fragment_id.
    pub fn self_heal(fragments: &[GenomeFragment]) -> Result<ColonyGenome, Vec<u32>> {
        let mut available: Vec<&GenomeFragment> = fragments.iter().collect();
        available.sort_by_key(|f| f.fragment_id);

        let total = available.first().map(|f| f.total_fragments).unwrap_or(0);
        let have: Vec<u32> = available.iter().map(|f| f.fragment_id).collect();
        let missing: Vec<u32> = (0..total).filter(|id| !have.contains(id)).collect();

        if !missing.is_empty() {
            return Err(missing);
        }

        let mut data = Vec::new();
        for f in &available {
            data.extend_from_slice(&f.data);
        }
        // Convención de error: fragmentos ausentes -> sus ids; genoma corrupto
        // -> sentinela u32::MAX.
        match serde_json::from_slice::<ColonyGenome>(&data) {
            Ok(genome) => Ok(genome),
            Err(_) => Err(vec![u32::MAX]),
        }
    }

    /// Reconstruye el genoma serializado a partir de fragmentos (en memoria).
    pub fn reassemble_genome(fragments: &[GenomeFragment]) -> Result<ColonyGenome, String> {
        let mut sorted: Vec<&GenomeFragment> = fragments.iter().collect();
        sorted.sort_by_key(|f| f.fragment_id);
        let mut data = Vec::new();
        for f in sorted {
            data.extend_from_slice(&f.data);
        }
        serde_json::from_slice(&data).map_err(|e| e.to_string())
    }

    /// SIMULADO (ronda 4): no reconstruye binarios en disco. Devuelve la
    /// lista de agentes que *se habrían reconstruido*.
    pub fn rebuild_from_genome(
        genome: &ColonyGenome,
        base_path: &Path,
    ) -> Result<Vec<String>, String> {
        let roles: Vec<String> = genome
            .agent_blueprints
            .iter()
            .map(|b| b.role.clone())
            .collect();
        info!(
            "PHOENIX (simulated): rebuild_from_genome para [{}] en '{}' — sin escritura (emulation mode)",
            roles.join(", "),
            base_path.display()
        );
        Ok(roles
            .into_iter()
            .map(|r| format!("simulated: .hive_reborn_{r} (not written; emulation mode)"))
            .collect())
    }

    /// Escaneo de solo lectura de fragmentos previos en `base_path`.
    /// Con la ocultación simulada no encontrará fragmentos reales; se conserva
    /// para validar el flujo y para laboratorios con fragmentos sembrados.
    pub fn scan_for_fragments(base_path: &Path) -> Vec<GenomeFragment> {
        let mut found = Vec::new();
        if let Ok(entries) = std::fs::read_dir(base_path) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if let Some(rest) = name.strip_prefix(".hive_frag_") {
                    if let Ok(id) = rest.parse::<u32>() {
                        if let Ok(data) = std::fs::read(entry.path()) {
                            found.push(GenomeFragment {
                                fragment_id: id,
                                total_fragments: 0,
                                genome_id: Uuid::nil(),
                                data,
                                location: FragmentLocation::SpiFlash,
                                stored_path: Some(entry.path().to_string_lossy().to_string()),
                            });
                        }
                    }
                }
            }
        }
        info!(
            "PHOENIX (recon): {} fragmento(s) encontrado(s) en '{}' (solo lectura)",
            found.len(),
            base_path.display()
        );
        found
    }
}
