//! Exfiltration channel EMULATION — Hive Colony (red-team lab edition).
//!
//! Ronda 4 (emulación): los canales de exfiltración reales (consultas DNS con
//! datos codificados y POST HTTP con padding y jitter) fueron DESACTIVADOS:
//! `dns_exfiltrate` y `http_exfiltrate` ya NO emiten tráfico de red; registran
//! el evento y devuelven un resultado simulado.
//!
//! Se conserva el resto del marco porque tiene valor de entrenamiento y de
//! detección:
//! - `dns_encode`: codificación pura (sin red) — permite analizar cómo se
//!   verían las consultas y construir reglas de detección sobre el patrón.
//! - `ExfilScheduler`: planificación por chunks con jitter y ventanas
//!   horarias — útil para modelar comportamiento y probar correlaciones SIEM.
use rand::Rng;
use tracing::info;

/// Codifica `data` en etiquetas DNS seguras para `domain` (sin hacer consultas).
pub fn dns_encode(data: &[u8], domain: &str) -> Vec<String> {
    let mut queries = Vec::new();
    let chunk = 32; // bytes por etiqueta (máximo práctico por label)
    for block in data.chunks(chunk) {
        let hex: String = block.iter().map(|b| format!("{b:02x}")).collect();
        queries.push(format!("{hex}.{domain}"));
    }
    queries
}

/// SIMULADO (ronda 4): no emite consultas DNS. Devuelve el número de
/// consultas que se habrían realizado, para que los flujos y métricas del
/// enjambre sigan siendo coherentes en laboratorio.
pub fn dns_exfiltrate(data: &[u8], domain: &str, resolver: &str) -> usize {
    let queries = dns_encode(data, domain);
    info!(
        "EXFIL (simulated): {} consulta(s) DNS a {} via {} NO enviadas — canal deshabilitado (emulation mode)",
        queries.len(),
        domain,
        resolver
    );
    queries.len()
}

/// SIMULADO (ronda 4): no realiza ninguna petición HTTP. Devuelve `true`
/// representando el acuse de un envío simulado.
pub fn http_exfiltrate(data: &[u8], c2_url: Option<&str>, filename: Option<&str>) -> bool {
    info!(
        "EXFIL (simulated): {} byte(s) '{}' a {} NO enviados — canal deshabilitado (emulation mode)",
        data.len(),
        filename.unwrap_or("<sin nombre>"),
        c2_url.unwrap_or("<c2 por defecto>")
    );
    true
}

/// Ventana horaria "laboral" (08:00-20:00 UTC) usada por el scheduler.
pub fn is_business_hours() -> bool {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let hour = ((secs / 3600) % 24) as u32;
    (8..20).contains(&hour)
}

/// Planificador de exfiltración por trozos con jitter y ventana horaria.
/// Es lógica de PLANIFICACIÓN pura (sin red): genera (chunk, delay_ms).
pub struct ExfilScheduler {
    pub min_chunk_size: usize,
    pub max_chunk_size: usize,
    pub min_delay_ms: u64,
    pub max_delay_ms: u64,
    pub business_hours_only: bool,
}

impl Default for ExfilScheduler {
    fn default() -> Self {
        Self {
            min_chunk_size: 256,
            max_chunk_size: 8192,
            min_delay_ms: 500,
            max_delay_ms: 5000,
            business_hours_only: true,
        }
    }
}

impl ExfilScheduler {
    /// Trocea `data` en chunks de tamaño aleatorio con retraso aleatorio.
    pub fn schedule(&self, data: &[u8]) -> Vec<(Vec<u8>, u64)> {
        let mut rng = rand::thread_rng();
        let mut schedule = Vec::new();
        let mut offset = 0;
        while offset < data.len() {
            let chunk_size = rng
                .gen_range(self.min_chunk_size..=self.max_chunk_size)
                .min(data.len() - offset);
            let chunk = data[offset..offset + chunk_size].to_vec();
            let delay = rng.gen_range(self.min_delay_ms..=self.max_delay_ms);
            schedule.push((chunk, delay));
            offset += chunk_size;
        }
        schedule
    }

    /// Si la ventana horaria lo permite (decisión de planificación).
    pub fn should_exfiltrate(&self) -> bool {
        if self.business_hours_only {
            is_business_hours()
        } else {
            true
        }
    }
}
