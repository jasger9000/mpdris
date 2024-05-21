use core::panic;
use std::process::Command;

fn main() {
    let hash = match Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
    {
        Ok(output) => String::from_utf8(output.stdout).expect("git output is utf-8"),
        _ => panic!("cannot get git hash"),
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
