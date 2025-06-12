use std::path::PathBuf;

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
}

#[derive(FromArgs, PartialEq, Debug)]
/// Write & compress manpages to <dir>
#[argh(subcommand, name = "man", help_triggers("-h", "--help"))]
pub(crate) struct ManTask {
    #[argh(positional)]
    /// the directory to which the compressed manpages should be written to
    pub(crate) dir: PathBuf,
}
