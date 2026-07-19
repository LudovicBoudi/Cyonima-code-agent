//! Test standalone : détecte le hardware (RAM + VRAM DXGI) et l'affiche.
//!
//! Usage : cargo run --manifest-path src-tauri/Cargo.toml \
//!         --example detect_hardware --release

fn main() {
    let hw = cyonima_core::hardware::detect();
    println!("=== Cyonima hardware detection ===");
    println!("OS             : {}", hw.os);
    println!("Arch           : {}", hw.arch);
    println!("CPU cores      : {}", hw.cpu_cores);
    println!(
        "RAM totale     : {} Go ({} octets)",
        hw.total_ram_gb, hw.total_ram_bytes
    );
    match hw.vram_bytes {
        Some(v) => println!("VRAM GPU max   : {} Go ({} octets)", hw.vram_gb, v),
        None => println!("VRAM GPU max   : non détectée"),
    }
    println!();
    println!("=== Tests can_run_model ===");
    for ram_min in [2u32, 4, 8, 12, 16, 20, 24, 32] {
        println!(
            "  ram_min_gb={ram_min:>2} -> {}",
            if hw.can_run_model(ram_min) {
                "OK"
            } else {
                "BLOQUÉ"
            }
        );
    }
}
