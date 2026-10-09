use anyhow::Result;
use license::{LicenseManager, Tier};

pub fn activate(key: &str) -> Result<()> {
    let lm = LicenseManager::new(license::CreemValidator::from_env()?);
    match lm.activate(key)? {
        Tier::Pro => {
            println!("License activated. Pro features unlocked.");
            Ok(())
        }
        _ => Ok(()),
    }
}

pub fn status() -> Result<()> {
    let lm = LicenseManager::new(license::CreemValidator::from_env().unwrap_or_else(|_| {
        license::CreemValidator::dummy()
    }));
    let st = lm.status();
    match st.tier {
        Tier::Pro => {
            println!("Status: Pro (activated)");
            println!("Features: target-size compression, unlimited batch, WebP/AVIF conversion");
        }
        Tier::Free => {
            println!("Status: Free");
            println!("Free limits: no --max target size, no WebP/AVIF, up to 10 images per run");
            println!("Upgrade: run 'imgcomp activate <KEY>' to unlock Pro.");
        }
    }
    Ok(())
}

pub fn deactivate() -> Result<()> {
    let lm = LicenseManager::new(license::CreemValidator::from_env().unwrap_or_else(|_| {
        license::CreemValidator::dummy()
    }));
    lm.deactivate()?;
    println!("License removed.");
    Ok(())
}

pub fn upgrade() -> Result<()> {
    println!("imgcomp Pro - one-time lifetime license (3 devices)");
    println!();
    println!("Pro unlocks:");
    println!("  - target-size compression (--max)");
    println!("  - unlimited batch");
    println!("  - WebP / AVIF conversion");
    println!();
    println!("Purchase:");
    println!("  1. store.creem.io -> imgcomp (Lifetime)");
    println!("  2. copy the license key from the receipt");
    println!("  3. run: imgcomp activate <KEY>");
    Ok(())
}
