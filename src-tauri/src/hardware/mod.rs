//! Détection hardware — évite qu'un utilisateur télécharge un modèle que sa
//! machine ne peut pas faire tourner.
//!
//! Critères évalués (J1.5) :
//! - **RAM totale** : comparée à `ram_min_gb` du catalogue de modèles.
//! - **VRAM GPU** : détectée via DXGI (Win), sysfs (Linux), system_profiler
//!   (macOS). Quand un modèle tient entièrement en VRAM, on relaxe la
//!   contrainte RAM car les poids ne consomment plus la RAM système.
//! - **OS / arch** : pour les binaires backend et les bundles Tauri.
//!
//! Si la VRAM n'est pas détectable, on garde la RAM totale comme seuil
//! conservateur. On ne devine JAMAIS une VRAM inconnue pour ne pas mentir à
//! `can_run_model`.

pub mod vram;

use serde::{Deserialize, Serialize};
use sysinfo::System;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    /// RAM totale en octets.
    pub total_ram_bytes: u64,
    /// RAM totale arrondie en Go (utile pour l'UI et les comparaisons).
    pub total_ram_gb: u32,
    /// Nombre de cœurs logiques.
    pub cpu_cores: u32,
    /// OS cible (`windows`, `macos`, `linux`).
    pub os: String,
    /// Architecture (`x86_64`, `aarch64`).
    pub arch: String,
    /// VRAM dédiée du GPU le plus capable (octets). `None` si non détectable.
    pub vram_bytes: Option<u64>,
    /// VRAM en Go arrondie. 0 si `vram_bytes` est `None`.
    pub vram_gb: u32,
}

impl HardwareInfo {
    /// Renvoie `true` si la machine peut a priori exécuter un modèle qui
    /// demande `ram_min_gb` de RAM.
    ///
    /// Règle (J1.5) :
    /// - Si le modèle tient entièrement en VRAM (poids ≤ VRAM), on relaxe à
    ///   `ram_min_gb - 1` (reste le contexte à héberger en RAM).
    /// - Sinon, seuil strict `ram_min_gb + 1` (marge OS/app).
    /// - Si VRAM non détectée, on reste conservateur avec le +1.
    pub fn can_run_model(&self, ram_min_gb: u32) -> bool {
        let ram_min_gb = if self.model_fits_vram(ram_min_gb) {
            ram_min_gb.saturating_sub(1)
        } else {
            ram_min_gb.saturating_add(1)
        };
        self.total_ram_gb >= ram_min_gb
    }

    /// Estime si un modèle dont `ram_min_gb` ≈ taille des poids tient en VRAM.
    /// Heuristique J1.5 : si VRAM ≥ ram_min_gb (Go), on considère que oui.
    /// Plus raffiné plus tard (taille du GGUF exact + mmap).
    fn model_fits_vram(&self, ram_min_gb: u32) -> bool {
        self.vram_gb > 0 && self.vram_gb >= ram_min_gb
    }
}

/// Snapshot hardware au moment de l'appel. Bon marché (< 1 ms en pratique
/// sur un `System::new_all()` déjà chaud, premier appel ~10-50 ms pour la
/// VRAM via DXGI/sysfs/system_profiler).
pub fn detect() -> HardwareInfo {
    let mut sys = System::new_all();
    sys.refresh_memory();

    let total_ram_bytes = sys.total_memory();
    // Arrondi au Go supérieur pour ne pas afficher un faux négatif au GiB près.
    let total_ram_gb = total_ram_bytes.div_ceil(1 << 30) as u32;

    let vram_bytes = vram::detect_vram();
    let vram_gb = vram_bytes
        .map(|b| (b.div_ceil(1 << 30)) as u32)
        .unwrap_or(0);

    HardwareInfo {
        total_ram_bytes,
        total_ram_gb,
        cpu_cores: sys.cpus().len() as u32,
        os: detect_os(),
        arch: detect_arch(),
        vram_bytes,
        vram_gb,
    }
}

fn detect_os() -> String {
    #[cfg(target_os = "windows")]
    return "windows".into();
    #[cfg(target_os = "macos")]
    return "macos".into();
    #[cfg(target_os = "linux")]
    return "linux".into();
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    return "unknown".into();
}

fn detect_arch() -> String {
    #[cfg(target_arch = "x86_64")]
    return "x86_64".into();
    #[cfg(target_arch = "aarch64")]
    return "aarch64".into();
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    return std::env::consts::ARCH.into();
}
