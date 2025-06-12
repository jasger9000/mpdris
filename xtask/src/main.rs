use std::path::{Path, PathBuf};
use std::{env, process::exit, sync::LazyLock};

use anyhow::Result;
use dist::{build, clean_dist, install};

pub(crate) use man::build_man;
pub(crate) use task::Task;

mod args;
mod dist;
mod man;
mod task;

static PROJECT_ROOT: LazyLock<PathBuf> = LazyLock::new(|| {
    Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(1)
        .unwrap()
        .to_path_buf()
});
static DIST_DIR: LazyLock<PathBuf> = LazyLock::new(|| PROJECT_ROOT.join("target/dist"));
static TARGET_DIR: LazyLock<PathBuf> = LazyLock::new(|| PROJECT_ROOT.join("target"));

const NAME: &str = "mpdris";
const MANPATH: &str = "resources/man";

fn main() {
    if let Err(e) = try_main() {
        eprintln!("{e:?}");
        exit(-1);
    }
}

fn try_main() -> Result<()> {
    use args::Task::*;

    let args: args::Args = argh::from_env();

    match args.task {
        Man(task) => build_man(&task.dir),
        Build(task) => build(task.path, &task.arch),
        Install(task) => install(task.path, &task.arch),
        CleanDist(..) => clean_dist(),
    }
}
