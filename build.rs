fn main() {
    println!("cargo::rustc-check-cfg=cfg(docker)");

    // Re-run when DOCKER_HOST changes or the socket file appears/disappears
    println!("cargo:rerun-if-env-changed=DOCKER_HOST");
    let sock = std::env::var("DOCKER_HOST")
        .ok()
        .and_then(|h| h.strip_prefix("unix://").map(String::from))
        .unwrap_or_else(|| "/var/run/docker.sock".to_string());
    println!("cargo:rerun-if-changed={sock}");

    let docker_running = std::process::Command::new("docker")
        .args(["info"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success());

    if docker_running {
        println!("cargo:rustc-cfg=docker");
    }
}
