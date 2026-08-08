#![cfg(unix)]
//! This test builds Vibe's actual Docker image and checks that its Pi
//! installation can read a custom model from models.json.

use std::process::{Command, Output};

struct DockerImage {
    name: String,
}

// Rust calls this automatically when the image value leaves scope.
impl Drop for DockerImage {
    fn drop(&mut self) {
        // Remove the temporary image even when a test assertion fails.
        // Ignore cleanup errors so they do not hide the original failure.
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

    // Build the same Docker image that Vibe uses.
    // Do not use a Pi installation from the developer's machine.
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

    // Start Pi directly because this test checks whether Pi can read
    // the mounted models.json file without calling a provider.
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
    // This provider and model only exist in the test file. Finding them in
    // Pi's output proves that Pi read the mounted configuration.
    let loaded = stdout.lines().any(|line| {
        let mut fields = line.split_whitespace();
        fields.next() == Some("vibe-fixture") && fields.next() == Some("dynamic-model")
    });

    assert!(loaded, "injected model was not loaded:\n{stdout}");
}
