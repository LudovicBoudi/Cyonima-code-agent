//! Détection VRAM GPU — cross-platform.
//!
//! Renvoie la VRAM dédiée du GPU le plus capable (en octets). Quand un modèle
//! tient entièrement en VRAM (poids < VRAM), le garde-fou `can_run_model` peut
//! relaxer sa contrainte RAM (passé à J1.5/J8) : la VRAM porte les poids et la
//! RAM système porte le contexte + l'app.
//!
//! Implémentations plateformes :
//! - **Windows** : DXGI `EnumAdapters1` + `GetDesc1.DedicatedVideoMemory`.
//!   C'est la méthode robuste : `Win32_VideoController` est buggé au-delà de
//!   4 Go (signé 32-bit). DXGI renvoie l'unsigned 64 correct.
//! - **Linux** : sysfs `/sys/class/drm/card*/device/` + `mem_info_vram_total`
//!   (amdgpu) ou heuristique fichier (drm). Fallback sur WMI absent.
//! - **macOS** : `system_profiler SPDisplaysDataType` parsé (le seul chemin
//!   stable sans Foundation binding). Retourne la VRAM du GPU dédié.
//!
//! N'importe quelle erreur de plateforme renvoie `None` — on retombe alors sur
//! le seuil conservateur RAM-only. C'est explicite : on ne devine JAMAIS une
//! VRAM inconnue pour ne pas mentir à `can_run_model`.

/// Renvoie la VRAM (en octets) du GPU le plus capable. `None` si non détecté.
pub fn detect_vram() -> Option<u64> {
    let candidates = platform_detect()?;
    candidates.into_iter().max()
}

#[cfg(target_os = "windows")]
#[allow(clippy::too_many_lines)]
fn platform_detect() -> Option<Vec<u64>> {
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1, DXGI_ERROR_NOT_FOUND};

    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }.ok()?;
    let mut out = Vec::new();
    for i in 0..16u32 {
        let adapter = match unsafe { factory.EnumAdapters1(i) } {
            Ok(a) => a,
            Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => break,
            Err(_) => break,
        };
        let desc = match unsafe { adapter.GetDesc1() } {
            Ok(d) => d,
            Err(_) => continue,
        };
        // DedicatedVideoMemory = VRAM dédiée en octets (u64 via usize sur x64).
        // On ignore le « Microsoft Basic Render Driver » (VRAM = 0).
        let vram: u64 = desc.DedicatedVideoMemory as u64;
        if vram > 0 {
            out.push(vram);
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

#[cfg(target_os = "linux")]
fn platform_detect() -> Option<Vec<u64>> {
    use std::fs;

    // 1) amdgpu / radeon : mem_info_vram_total en octets
    //    /sys/class/drm/card*/device/mem_info_vram_total
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir("/sys/class/drm") {
        for entry in entries.flatten() {
            let p = entry.path();
            // N'évalue que cardN (pas cardN-* qui sont sorties HDMI/DP)
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with("card") || name.contains('-') {
                continue;
            }
            let vram_file = p.join("device").join("mem_info_vram_total");
            if let Ok(s) = fs::read_to_string(&vram_file) {
                if let Ok(v) = s.trim().parse::<u64>() {
                    if v > 0 {
                        out.push(v);
                    }
                }
            }
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

#[cfg(target_os = "macos")]
fn platform_detect() -> Option<Vec<u64>> {
    use std::process::Command;

    // system_profiler SPDisplaysDataType renvoie du texte structuré :
    //   VRAM (Total): 16 GB
    // On vérifie uniquement les GPUs dédiés ; Apple Silicon a une VRAM unifiée
    // (égale à la RAM), qu'on n'expose pas ici pour rester honnête.
    let out = Command::new("system_profiler")
        .arg("SPDisplaysDataType")
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut vrams = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("VRAM") {
            continue;
        }
        let Some((_, val)) = trimmed.split_once(':') else {
            continue;
        };
        let val = val.trim();
        // Formats : "16 GB", "16384 MB", "8 GB"
        let bytes = parse_vram_string(val)?;
        vrams.push(bytes);
    }
    if vrams.is_empty() {
        None
    } else {
        Some(vrams)
    }
}

#[cfg(target_os = "macos")]
fn parse_vram_string(s: &str) -> Option<u64> {
    let s = s.trim();
    let (num_part, unit) = s.split_once(' ').unwrap_or((s, "GB"));
    let n: f64 = num_part.parse().ok()?;
    let bytes = match unit.to_ascii_uppercase().as_str() {
        "GB" => n * (1u64 << 30) as f64,
        "MB" => n * (1u64 << 20) as f64,
        "KB" => n * (1u64 << 10) as f64,
        "B" | "" => n,
        _ => return None,
    };
    Some(bytes as u64)
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn platform_detect() -> Option<Vec<u64>> {
    None
}
