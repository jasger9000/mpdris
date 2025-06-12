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
    CleanDist(CleanTask),
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
/// Clean the target/dist directory
#[argh(subcommand, name = "clean-dist", help_triggers("-h", "--help"))]
pub(crate) struct CleanTask {}
