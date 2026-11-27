//! Embeds the current git SHA into the binary so that bench reports can be
//! attributed to a revision without shelling out at bench runtime.
//! Override with the `ELECTRICAL_BENCH_GIT_SHA` environment variable at build
//! time (e.g. when building outside a git checkout).

fn main() {
    println!("cargo:rerun-if-env-changed=ELECTRICAL_BENCH_GIT_SHA");

    let sha = std::env::var("ELECTRICAL_BENCH_GIT_SHA").unwrap_or_else(|_| {
        std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|sha| sha.trim().to_owned())
            .filter(|sha| !sha.is_empty())
            .unwrap_or_else(|| "unknown".to_owned())
    });

    println!("cargo:rustc-env=ELECTRICAL_BENCH_GIT_SHA={sha}");
}
