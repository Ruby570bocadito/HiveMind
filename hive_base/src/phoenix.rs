//! Phoenix — colony genome modeling (in memory).
//!
//! Ronda 6: las APIs de ocultación, persistencia y reconstrucción en disco
//! fueron ELIMINADAS del repositorio (histórico: emulación en ronda 4,
//! eliminación en ronda 6). Se conserva exclusivamente el modelado del
//! genoma de la colonia (`ColonyGenome`, `AgentBlueprint`, `GenomeFragment`,
//! `FragmentLocation`) y sus funciones puras en memoria (`generate_genome`,
//! `fragment_genome`, `reassemble_genome`, `self_heal`), que permiten probar
//! recuperación topológica sin tocar disco. El campo `location` es
//! taxonomía documental heredada; no corresponde a ninguna escritura real.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
        config.insert("lab_mode".into(), "true".into());

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
            });
        }

        fragments
    }
    /// SIMULADO (ronda 4): los fragmentos ya no se cifran al esconderse, así
    /// que la recuperación devuelve los datos tal cual (operación en memoria).
    pub fn recover(fragment: &GenomeFragment) -> Result<Vec<u8>, String> {
        Ok(fragment.data.clone())
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

}
