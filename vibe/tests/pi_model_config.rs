#![cfg(unix)]
//! This test builds Vibe's Docker image and checks Pi reads models.json.

use std::process::{Command, Output};

struct DockerImage {
    name: String,
}

impl Drop for DockerImage {
    fn drop(&mut self) {
        let _ = Command::new("docker")
            .args(["image", "rm", "--force", &self.name])
            .output();
    }
}

fn bash(script: &str, env: &[(&str, &str)]) -> Output {
    let mut command = Command::new("bash");
    command.args(["-euo", "pipefail", "-c", script]);

    for &(name, value) in env {
        command.env(name, value);
    }

    command.output().expect("execute integration command")
}

#[test]
#[ignore = "requires Docker and network access"]
fn pi_loads_injected_model_config() {
    let root = env!("CARGO_MANIFEST_DIR");
    let fixture = format!("{root}/tests/fixtures/pi-agent");
    let image = DockerImage {
        name: format!("vibe-pi-integration-{}", std::process::id()),
    };

    let build = bash(
        r#"
docker build \
  --tag "$IMAGE" \
  --file "$ROOT/docker/Dockerfile" \
  "$ROOT"
"#,
        &[("IMAGE", &image.name), ("ROOT", root)],
    );

    assert!(
        build.status.success(),
        "Docker build failed:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let lookup = bash(
        r#"
docker run --rm \
  --entrypoint pi \
  --env HOME=/vibe-home \
  --volume "$FIXTURE:/vibe-home/.pi/agent:ro" \
  "$IMAGE" \
  --list-models dynamic-model
"#,
        &[("IMAGE", &image.name), ("FIXTURE", &fixture)],
    );

    assert!(
        lookup.status.success(),
        "Pi model lookup failed:\n{}",
        String::from_utf8_lossy(&lookup.stderr)
    );

    let stdout = String::from_utf8_lossy(&lookup.stdout);
    let loaded = stdout.lines().any(|line| {
        let mut fields = line.split_whitespace();
        fields.next() == Some("vibe-fixture") && fields.next() == Some("dynamic-model")
    });

    assert!(loaded, "injected model was not loaded:\n{stdout}");
}
