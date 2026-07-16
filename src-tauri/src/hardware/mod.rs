//! Détection hardware — évite qu'un utilisateur télécharge un modèle que sa
//! machine ne peut pas faire tourner.
//!
//! Critères évalués pour l'instant (v1) :
//! - **RAM totale** : comparée à `ram_min_gb` du catalogue de modèles.
//! - **OS / arch** : pour les binaires backend et les bundles Tauri.
//!
//! Critères prévus plus tard (post-v1) : VRAM GPU détectée via wgpu/Vulkan,
//! accélération Metal/CUDA, fallback CPU automatique. Si la VRAM n'est pas
//! détectable, on garde la RAM totale comme seuil conservateur.

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
    /// Détection VRAM GPU (en octets). `None` si non détectable en v1.
    pub vram_bytes: Option<u64>,
}

impl HardwareInfo {
    /// Renvoie `true` si la machine peut a priori exécuter un modèle qui
    /// demande `ram_min_gb` de RAM. Ajoute une marge de 1 Go pour l'OS et le
    /// reste de l'app afin d'éviter les estimations trop justes.
    pub fn can_run_model(&self, ram_min_gb: u32) -> bool {
        self.total_ram_gb >= ram_min_gb.saturating_add(1)
    }
}

/// Snapshot hardware au moment de l'appel. Bon marché (< 1 ms en pratique
/// sur un `System::new_all()` déjà chaud, premier appel ~10-50 ms).
pub fn detect() -> HardwareInfo {
    let mut sys = System::new_all();
    sys.refresh_memory();

    let total_ram_bytes = sys.total_memory();
    // Arrondi au Go supérieur pour ne pas afficher un faux négatif au GiB près.
    let total_ram_gb = total_ram_bytes.div_ceil(1 << 30) as u32;

    HardwareInfo {
        total_ram_bytes,
        total_ram_gb,
        cpu_cores: sys.cpus().len() as u32,
        os: detect_os(),
        arch: detect_arch(),
        vram_bytes: None, // TODO post-v1 : wgpu / Vulkan / Metal
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
