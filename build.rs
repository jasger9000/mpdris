use core::panic;
use git2::{ObjectType, Repository};
use std::{env, process::Command};

fn main() {
    println!("cargo::rerun-if-changed={}", env::var("RUSTC").expect("$RUSTC is not valid"));
    println!("cargo::rerun-if-env-changed=GIT_HASH");

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
        }
    }

    let hash = {
        if let Ok(var) = env::var("GIT_HASH") {
            var
        } else {
            match Command::new("git").args(["rev-parse", "--short", "HEAD"]).output() {
                Ok(output) => String::from_utf8(output.stdout).expect("git output is utf-8"),
                _ => panic!("cannot get git hash"),
            }
        }
    };

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

    println!("cargo::rustc-env=GIT_HASH={hash}");
    println!("cargo::rustc-env=RUSTC_VERSION={rustc_ver}");
}
