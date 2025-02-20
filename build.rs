use core::panic;
use git2::{ObjectType, Repository};
use std::{env, process::Command};

fn main() {
    println!("cargo::rerun-if-changed={}", env::var("RUSTC").expect("$RUSTC is not valid"));
    println!("cargo::rerun-if-env-changed=GIT_HASH");

    if let Ok(var) = env::var("GIT_HASH") {
        println!("cargo::rustc-env=GIT_HASH={var}");
    }

    if let Ok(repo) = Repository::open(".") {
        // add .git/HEAD to rerun-if
        let head_path = repo.path().join("HEAD");
        if head_path.exists() {
            println!("cargo::rerun-if-changed={}", head_path.display());
        }

        if let Ok(head) = repo.head() {
            // add the ref that HEAD points to to rerun-if
            if let Ok(pointer) = head.resolve() {
                if let Some(name) = pointer.name() {
                    let path = repo.path().join(name);
                    println!("cargo::rerun-if-changed={}", path.display());
                }
            }

            // emit the git hash if not overriden
            if env::var_os("GIT_HASH").is_none() {
                println!(
                    "cargo::rustc-env=GIT_HASH={}",
                    head.peel(ObjectType::Commit)
                        .expect("failed to get last commit")
                        .short_id()
                        .expect("failed to get commit SHA")
                        .as_str()
                        .expect("failed to turn commit SHA into str")
                );
            }
        }
    }

    let rustc_ver = match Command::new("rustc").arg("-vV").output() {
        Ok(output) => {
            let mut ver = None;
            for line in String::from_utf8(output.stdout)
                .expect("rustc output is utf-8")
                .lines()
                .skip(1)
            {
                let mut split = line.split(": ");
                if let (Some(k), Some(v)) = (split.next(), split.next()) {
                    if k == "release" {
                        ver = Some(v.to_string());
                        break;
                    };
                } else {
                    unreachable!("rustc -vV always outputs key-value pairs");
                }
            }

            ver.expect("rustc -vV always outputs its version")
        }
        _ => panic!("Cannot get output from rustc"),
    };

    println!("cargo::rustc-env=RUSTC_VERSION={rustc_ver}");
}
