use std::fs::{self, File, Permissions, create_dir_all};
use std::io::{BufWriter, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use flate2::{Compression, write::GzEncoder};

use crate::{MANPATH, PROJECT_ROOT, Task};

pub(crate) fn build_man(outdir: &Path) -> Result<()> {
    let indir = PROJECT_ROOT.join(MANPATH);
    let skip = indir.components().count();

    if outdir.exists() {
        let t = Task::new("Removing old manpage output directory");
        fs::remove_dir_all(outdir).with_context(|| "Failed to delete manpage output directory")?;
        fs::create_dir_all(outdir).with_context(|| "Failed to create manpage output directory")?;
        t.success();
    }

    let t = Task::new("Building man pages");
    for inpath in search_files_recursive(&indir)? {
        let mut outpath: PathBuf = outdir.components().chain(inpath.components().skip(skip)).collect();
        outpath.as_mut_os_string().push(".gz");

        create_dir_all(outpath.parent().with_context(|| "Failed to get manpage output directory")?)
            .with_context(|| "Failed to create manpage output directory")?;

        let infile = fs::read(&inpath).with_context(|| "Failed to read manpage infile")?;
        let writer = BufWriter::new(File::create(&outpath).with_context(|| "Failed to open manpage outfile")?);

        let mut encoder = GzEncoder::new(writer, Compression::new(9));
        encoder.write_all(&infile)?;
        encoder.try_finish()?;

        fs::set_permissions(&outpath, Permissions::from_mode(0o644)).with_context(|| "Failed to set manpage permissions")?;
    }

    t.success();
    Ok(())
}

/// Finds all files present in a given start directory,
fn search_files_recursive(start: &Path) -> Result<Vec<PathBuf>> {
    fn inner(start: &Path, result: &mut Vec<PathBuf>) -> Result<()> {
        if start.is_dir() {
            for entry in start.read_dir().with_context(|| "failed to read dir")? {
                let entry = entry.with_context(|| "failed to get entry")?;
                let path = entry.path();

                if path.is_dir() {
                    inner(&path, result)?;
                } else {
                    result.push(path);
                }
            }
        }

        Ok(())
    }

    let mut result: Vec<PathBuf> = Vec::new();
    inner(start, &mut result)?;

    Ok(result)
}
