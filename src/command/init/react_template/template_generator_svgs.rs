use std::fs;
use std::path::PathBuf;
use anyhow::Result;

pub fn create_placeholder_svgs(target_dir: &PathBuf) -> Result<()> {
    // Simple placeholder SVGs
    let react_svg = r#"<svg width="32" height="32" viewBox="0 0 32 32"><circle cx="16" cy="16" r="12" fill="#61dafb"/><text x="16" y="20" text-anchor="middle" fill="white" font-size="8">React</text></svg>"#;
    fs::write(target_dir.join("src/assets/react.svg"), react_svg)?;

    let apollo_svg = r#"<svg width="32" height="32" viewBox="0 0 32 32"><circle cx="16" cy="16" r="12" fill="#311c87"/><text x="16" y="20" text-anchor="middle" fill="white" font-size="7">Apollo</text></svg>"#;
    fs::write(target_dir.join("src/assets/apollo.svg"), apollo_svg)?;

    let vite_svg = r#"<svg width="32" height="32" viewBox="0 0 32 32"><circle cx="16" cy="16" r="12" fill="#646cff"/><text x="16" y="20" text-anchor="middle" fill="white" font-size="8">Vite</text></svg>"#;
    fs::write(target_dir.join("public/vite.svg"), vite_svg)?;

    Ok(())
}