# OSS Scanner

This directory prepares KubeCove for [Anthropic OSS Scanner](https://red.anthropic.com/oss-scanner/). It does not enroll the project or establish eligibility. Anthropic accepts established, security-critical open-source projects case by case. Reports are model-generated and arrive without human review; maintainers must reproduce findings and review patches privately.

## Build and verify offline

Run from the repository root with Docker and BuildKit available:

```sh
docker build --progress=plain -f .oss-scanner/Dockerfile -t kubecove:oss-scanner .
docker run --rm --network=none kubecove:oss-scanner bun test
docker run --rm --network=none kubecove:oss-scanner cargo test --locked --workspace --manifest-path src-tauri/Cargo.toml
docker run --rm --network=none kubecove:oss-scanner cargo test --locked --manifest-path src-tauri/server/Cargo.toml
```

The image keeps the checkout at `/src`, with Bun, Rust, Linux desktop libraries, both Cargo dependency caches, and debug build/test artifacts. It builds the native frontend and desktop binary, then the browser frontend and headless server. The compile steps run with networking disabled. Source and compiler caches stay in the image so the scanner can change and rebuild reproducers offline. The final image retains `apt` for Anthropic's tools layer. It is an audit environment, not a deployment image.

`Dockerfile.dockerignore` replaces the production image's source allowlist for this build. It includes the vendored updater patch, tests, fixtures, and documentation. It excludes host profiles, kubeconfig, `.env` files, Git configuration, caches, and local artifacts. Build from a clean checkout and never mount real credentials, the Docker socket, or a live cluster into the audit container.

This Linux environment does not prove macOS or Windows behavior. Desktop GUI E2E and real-cluster workflows are not part of the offline checks. See the [threat model](threat_model.md) for synthetic testing and scope.

## Enroll after merge

1. Merge these files into KubeCove's `main` branch. The scanner clones the branch named in [project.yaml](project.yaml); it cannot use unmerged local files.
2. Review Anthropic's [current enrollment instructions](https://github.com/anthropics/oss-scanner#how-to-sign-up) and [terms](https://red.anthropic.com/oss-scanner/terms). A core maintainer must submit the enrollment and complete its terms and CLA steps.
3. In a fork of `anthropics/oss-scanner`, copy this directory's `project.yaml` to `projects/kubecove/project.yaml`. The Dockerfile and threat model remain in KubeCove; do not also copy them beside that file. The contact email is public and is listed in [SECURITY.md](../SECURITY.md).
4. Run the upstream `tools/validate.py` and `tools/check kubecove` using that project's documented prerequisites. Where Linux x64 and QEMU are available, use `tools/check --qemu kubecove` to check the reference VM environment as well. Then open the enrollment PR. These checks validate the upstream submission and the scanner's added tools layer. The direct Docker commands above only verify KubeCove's image.

Suggested enrollment rationale: KubeCove handles Kubernetes credentials, authenticated remote sessions, Pod exec and port forwarding, and guarded cluster operations. Vulnerabilities in these boundaries could expose cluster credentials or permit unintended operations.

The scan cadence is controlled by Anthropic. Keep the existing Bun and RustSec dependency audits and E2E checks. Triage reports through the private process in `SECURITY.md`; do not automatically publish findings or apply proposed patches.
