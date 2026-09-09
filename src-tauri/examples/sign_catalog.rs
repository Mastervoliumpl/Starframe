use std::{num::NonZeroU64, path::PathBuf};
use tough::{
    editor::{RepositoryEditor, signed::PathExists},
    key_source::LocalKeySource,
};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 6 {
        return Err("Usage: sign_catalog ROOT KEY INPUT OUTPUT VERSION EXPIRY".into());
    }
    let input = PathBuf::from(&args[2]);
    let output = PathBuf::from(&args[3]);
    if output.exists() {
        return Err("Signing output must be a new directory.".into());
    }
    let version: NonZeroU64 = args[4].parse()?;
    let mut editor = RepositoryEditor::new(&args[0]).await?;
    editor
        .targets_version(version)?
        .targets_expires(args[5].parse()?)?
        .snapshot_version(version)
        .snapshot_expires(args[5].parse()?)
        .timestamp_version(version)
        .timestamp_expires(args[5].parse()?);
    for name in ["catalog.json", "advisories.json"] {
        editor.add_target_path(input.join(name)).await?;
    }
    let signed = editor
        .sign(&[Box::new(LocalKeySource {
            path: args[1].clone().into(),
        })])
        .await?;
    signed
        .copy_targets(&input, output.join("targets"), PathExists::Fail)
        .await?;
    signed.write(output.join("metadata")).await?;
    Ok(())
}
