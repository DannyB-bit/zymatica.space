use anyhow::{Context, Result};
use std::fs::File;
use std::path::Path;

#[cfg(not(target_family = "wasm"))]
pub use memmap2::Mmap;

#[cfg(target_family = "wasm")]
#[derive(Debug, Clone)]
pub struct Mmap(pub std::sync::Arc<Vec<u8>>);

#[cfg(target_family = "wasm")]
impl std::ops::Deref for Mmap {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(not(target_family = "wasm"))]
pub fn map_read_only(path: &Path) -> Result<Mmap> {
    use memmap2::MmapOptions;
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut options = MmapOptions::new();
    if mmap_populate_enabled() {
        options.populate();
    }

    // SAFETY: all callers use read-only mappings for immutable model/cache artifacts.
    let mmap = unsafe { options.map(&file) }
        .with_context(|| format!("memory-mapping {}", path.display()))?;
    advise_large_read_only_mapping(&mmap);
    Ok(mmap)
}

#[cfg(target_family = "wasm")]
pub fn map_read_only(path: &Path) -> Result<Mmap> {
    use std::io::Read;
    let mut file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(Mmap(std::sync::Arc::new(bytes)))
}

#[cfg(not(target_family = "wasm"))]
pub fn advise_large_read_only_mapping(mmap: &Mmap) {
    #[cfg(unix)]
    {
        let _ = mmap.advise(memmap2::Advice::WillNeed);
    }
    #[cfg(not(unix))]
    {
        let _ = mmap;
    }
}

#[cfg(not(target_family = "wasm"))]
fn mmap_populate_enabled() -> bool {
    env_flag("ZYMATICA_MMAP_POPULATE")
}

#[allow(dead_code)]
#[cfg(all(unix, not(target_family = "wasm")))]
fn mmap_hugepage_advice_enabled() -> bool {
    env_flag_default("ZYMATICA_MMAP_HUGEPAGE", true)
}

#[allow(dead_code)]
#[cfg(all(unix, not(target_family = "wasm")))]
fn mmap_willneed_advice_enabled() -> bool {
    env_flag("ZYMATICA_MMAP_WILLNEED")
}

#[cfg(not(target_family = "wasm"))]
fn env_flag(name: &str) -> bool {
    env_flag_default(name, false)
}

#[cfg(not(target_family = "wasm"))]
fn env_flag_default(name: &str, default: bool) -> bool {
    match std::env::var(name) {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => default,
    }
}
