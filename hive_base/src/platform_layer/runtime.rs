//! Runtime environment utilities (platform-gated).
//!
//! Ronda 12: las primitivas de anti-análisis (`detect_sandbox`,
//! `detect_debugger`, `detect_edr`, `evasion_check`) fueron ELIMINadas —
//! su único consumidor era el gate de OPSEC (`opsec::should_act`), que
//! congelaba beacons al detectar sandbox/debugger/EDR: comportamiento de
//! evasión de la era ofensiva, incoherente con la matriz de CAPABILITIES
//! ("sin anti-análisis"). La colonia de laboratorio no altera su
//! comportamiento según quién la observa; la detección de EDR/backup con
// fines de INFORME (solo lectura, publicada como telemetría) vive en
//! `system_info.rs`.
//!
//! (Nota: los comentarios de arriba usan ELIMINADAS/ELIMINADOS sin acento
//! a propósito — varios terminales del lab no renderizan acentos.)
