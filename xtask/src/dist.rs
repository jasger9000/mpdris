use std::fs;
use std::path::{Path, PathBuf};
use std::{env, io::Write, os::unix::fs::PermissionsExt, process::Command, sync::Arc};

use crate::{DIST_DIR, NAME, PROJECT_ROOT, TARGET_DIR, Task, build_man};
use anyhow::{Context, Result, anyhow};
macro_rules! cp {
    ($indir:expr, $outdir:expr, $src:literal, $dst:literal, $perm:expr) => {
        $crate::dist::copy($indir, $outdir, &::std::format!($src), &::std::format!($dst), $perm)
    };
}

fn copy<P: AsRef<Path>>(indir: &Path, outdir: &Path, src: P, dst: P, perm: u32) -> Result<()> {
    let (src, dst) = (indir.join(src), outdir.join(dst));
    fs::copy(&src, &dst).with_context(|| format!("Failed to copy {}", dst.file_name().unwrap().display()))?;
    fs::set_permissions(&dst, Permissions::from_mode(perm))?;
    Ok(())
}

pub(crate) fn clean_dist() -> Result<()> {
    let t = Task::new("Cleaning dist");

    if DIST_DIR.exists() {
        fs::remove_dir_all(&*DIST_DIR).with_context(|| "Failed to delete the dist directory")?;
    }

    t.success();
    Ok(())
}

pub(crate) fn build_binary(arch: &str) -> Result<()> {
    let t = Arc::new(Task::new(&format!("Compiling binary for {arch}")));
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    println!();
    let status = Command::new(cargo)
        .current_dir(&*PROJECT_ROOT)
        .env("CARGO_TARGET_DIR", &*TARGET_DIR)
        .args([
            "build",
            "--frozen",
            "--release",
            &format!("--target={arch}-unknown-linux-gnu"),
        ])
        .status()
        .with_context(|| "Failed to execute build command")?;
    t.fix_text();

    if !status.success() {
        t.failure();
        return Err(anyhow!("Failed to compile binary"));
    }
    t.success();

    let t = Task::new("Copying binary to dist");
    fs::create_dir_all(&*DIST_DIR).with_context(|| "Failed to create dist directory")?;
    #[rustfmt::skip]
    cp!(TARGET_DIR, DIST_DIR, "{arch}-unknown-linux-gnu/release/{NAME}", "{NAME}_{arch}-linux-gnu");
    t.success();

    Ok(())
}

pub(crate) fn build(path: Option<PathBuf>, arch: &str) -> Result<()> {
    let outdir = path.unwrap_or(DIST_DIR.to_path_buf());

    build_binary(arch)?;
    build_man(&outdir.join("man"))?;

    Ok(())
}
