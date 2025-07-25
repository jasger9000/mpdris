use std::{env, path::PathBuf};

use argh::FromArgs;

/// XTasks
#[derive(FromArgs)]
#[argh(help_triggers("-h", "--help"))]
pub(crate) struct Args {
    #[argh(subcommand)]
    /// the task to execute
    pub(crate) task: Task,
}

#[derive(FromArgs)]
#[argh(subcommand)]
pub(crate) enum Task {
    Man(ManTask),
    Build(BuildTask),
    Install(InstallTask),
    CleanDist(CleanTask),
    MakeRelease(ReleaseTask),
}

#[derive(FromArgs, PartialEq, Debug)]
/// Write & compress manpages to <dir>
#[argh(subcommand, name = "man", help_triggers("-h", "--help"))]
pub(crate) struct ManTask {
    #[argh(positional)]
    /// the directory to which the compressed manpages should be written to
    pub(crate) dir: PathBuf,
}

#[derive(FromArgs, PartialEq, Debug)]
/// Compile/Build all project assets for the default arch or the one provided.
/// Result is written to target/dist/<arch> or <path> if provided.
#[argh(subcommand, name = "build", help_triggers("-h", "--help"))]
pub(crate) struct BuildTask {
    #[argh(option, default = "env::consts::ARCH.to_string()")]
    /// the arch to compile for
    pub(crate) arch: String,
    #[argh(positional)]
    /// path to install the files to instead of target/dist/<arch>
    pub(crate) path: Option<PathBuf>,
}

#[derive(FromArgs, PartialEq, Debug)]
/// Create an install using the default arch or <arch> if provided.
/// Result is written to target/dist/<arch> or <path> if provided.
/// Note: install does NOT compile anything, for that please use build
#[argh(subcommand, name = "install", help_triggers("-h", "--help"))]
pub(crate) struct InstallTask {
    #[argh(option, default = "env::consts::ARCH.to_string()")]
    /// the arch to compile for
    pub(crate) arch: String,
    #[argh(positional)]
    /// path to install the files to instead of target/dist/<arch>
    pub(crate) path: Option<PathBuf>,
}

#[derive(FromArgs, PartialEq, Debug)]
/// Clean the target/dist directory
#[argh(subcommand, name = "clean-dist", help_triggers("-h", "--help"))]
pub(crate) struct CleanTask {}

#[derive(FromArgs, PartialEq, Debug)]
/// Create release assets (tarballs, binaries and SHA256 checksums) for x86_64, aarch64, i68
#[argh(subcommand, name = "make-release-assets", help_triggers("-h", "--help"))]
pub(crate) struct ReleaseTask {}
