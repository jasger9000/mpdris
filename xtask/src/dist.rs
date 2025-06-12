use std::fs;
use std::path::{Path, PathBuf};
use std::{env, fs::Permissions, io::Write, os::unix::fs::PermissionsExt, process::Command, sync::Arc};

use crate::{DIST_DIR, NAME, PROJECT_ROOT, TARGET_DIR, Task, build_man};
use anyhow::{Context, Result, anyhow};
use flate2::{Compression, write::GzEncoder};
use sha2::{Digest, Sha256};

macro_rules! cp {
    ($outdir:expr, $src:literal, $dst:literal, $perm:expr) => {
        cp!(&$crate::PROJECT_ROOT, $outdir, $src, $dst, $perm)
    };
    ($indir:expr, $outdir:expr, $src:literal, $dst:literal, $perm:expr) => {
        $crate::dist::copy($indir, $outdir, &::std::format!($src), &::std::format!($dst), $perm)
    };
    ($indir:expr, $outdir:expr, $src:literal, $dst:literal) => {
        $crate::dist::copy_dir_all($indir.join(::std::format!($src)), $outdir.join(::std::format!($dst)))
    };
}

fn copy<P: AsRef<Path>>(indir: &Path, outdir: &Path, src: P, dst: P, perm: u32) -> Result<()> {
    let (src, dst) = (indir.join(src), outdir.join(dst));
    fs::copy(&src, &dst).with_context(|| format!("Failed to copy {}", dst.file_name().unwrap().display()))?;
    fs::set_permissions(&dst, Permissions::from_mode(perm))?;
    Ok(())
}

fn copy_dir_all<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) -> Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            copy_dir_all(path, dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(path, dst.as_ref().join(entry.file_name()))?;
        }
    }
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
    cp!(&TARGET_DIR, &DIST_DIR, "{arch}-unknown-linux-gnu/release/{NAME}", "{NAME}_{arch}-linux-gnu", 0o755)?;
    t.success();

    Ok(())
}

pub(crate) fn build(path: Option<PathBuf>, arch: &str) -> Result<()> {
    let outdir = path.unwrap_or(DIST_DIR.to_path_buf());

    build_binary(arch)?;
    build_man(&outdir.join("man"))?;

    Ok(())
}

pub(crate) fn install(path: Option<PathBuf>, arch: &str) -> Result<()> {
    let outdir = path.unwrap_or(DIST_DIR.join(arch));

    install_create_dirs(&outdir).with_context(|| "Failed to create dist directory structure")?;
    install_copy_files(&outdir, arch).with_context(|| "Failed to copy assets to install dir")?;

    Ok(())
}

fn install_create_dirs(outdir: &Path) -> Result<()> {
    let t = Task::new("Creating directory structure");
    fs::create_dir_all(outdir)?;
    fs::create_dir_all(outdir.join("usr/bin"))?;
    fs::create_dir_all(outdir.join("usr/lib/systemd/user"))?;
    fs::create_dir_all(outdir.join(format!("usr/share/doc/{NAME}")))?;
    fs::create_dir_all(outdir.join(format!("usr/share/licenses/{NAME}")))?;
    fs::create_dir_all(outdir.join("usr/share/man"))?;

    t.success();
    Ok(())
}

#[rustfmt::skip]
fn install_copy_files(outdir: &Path, arch: &str) -> Result<()> {
    let t = Task::new("Copying files to dist");
    cp!(&DIST_DIR, outdir, "{NAME}_{arch}-linux-gnu", "usr/bin/{NAME}", 0o755)?;
    cp!(outdir, "resources/mpdris.service", "usr/lib/systemd/user/mpdris.service", 0o644)?;
    cp!(outdir, "resources/sample.mpdris.conf", "usr/share/doc/{NAME}/sample.mpdris.conf", 0o644)?;
    cp!(outdir, "README.md", "usr/share/doc/{NAME}/README.md", 0o644)?;
    cp!(outdir, "LICENSE", "usr/share/licenses/{NAME}/LICENSE", 0o644)?;
    cp!(DIST_DIR, outdir, "man", "usr/share/man")?;

    t.success();
    Ok(())
}

pub(crate) fn make_release_assets() -> Result<()> {
    let archs = ["x86_64", "i686", "aarch64"];
    let mandir = DIST_DIR.join("man");
    let mut checksums = (Vec::new(), Vec::new());

    if !DIST_DIR.is_dir() {
        fs::create_dir_all(&*DIST_DIR).with_context(|| "Failed to create dist directory")?;
    }
    build_man(&mandir)?;

    for arch in archs {
        println!("Making release for {arch}");

        build_binary(arch)?;
        let tarball_filename = format!("{NAME}_{arch}.tar.gz");
        let binary_filename = format!("{NAME}_{arch}-linux-gnu");
        let binary_outpath = PROJECT_ROOT.join(DIST_DIR.join(&binary_filename));

        let t = Task::new("Making tar archive");
        let mut builder = tar::Builder::new(Vec::new());
        builder.mode(tar::HeaderMode::Deterministic);
        builder.append_path_with_name(&binary_outpath, NAME)?;
        builder.append_path_with_name(PROJECT_ROOT.join("resources/mpdris.service"), "mpdris.service")?;
        builder.append_path_with_name(PROJECT_ROOT.join("resources/mpdris.service.local"), "mpdris.service.local")?;
        builder.append_path_with_name(PROJECT_ROOT.join("resources/sample.mpdris.conf"), "sample.mpdris.conf")?;
        builder.append_path_with_name(PROJECT_ROOT.join("README.md"), "README.md")?;
        builder.append_path_with_name(PROJECT_ROOT.join("LICENSE"), "LICENSE")?;
        builder.append_dir_all("man", &mandir)?;

        let archive = builder.into_inner()?;
        t.success();

        let t = Task::new("Compressing archive");
        let mut encoder = GzEncoder::new(Vec::new(), Compression::new(9));
        encoder.write_all(&archive)?;

        let compressed = encoder.finish()?;
        drop(archive);
        t.success();

        let t = Task::new("Calculating checksums");
        let binary_hash = hex::encode(Sha256::digest(fs::read(&binary_outpath)?));
        let archive_hash = hex::encode(Sha256::digest(&compressed));
        checksums.0.push(format!("{binary_hash} {binary_filename}"));
        checksums.1.push(format!("{archive_hash} {tarball_filename}"));
        t.success();

        let t = Task::new("Writing tarball");
        fs::write(DIST_DIR.join(tarball_filename), compressed).with_context(|| "failed to write compressed archive")?;
        t.success();
        println!();
    }

    let t = Task::new("Writing checksum file");
    checksums.0.append(&mut checksums.1);
    fs::write(DIST_DIR.join("SHA256sums.txt"), checksums.0.join("\n").as_bytes())?;
    t.success();

    Ok(())
}
