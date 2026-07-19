# cudaenv

`cudaenv` is a GPU environment manager with a provider-oriented architecture.
The current NVIDIA provider inspects NVIDIA GPU environments and installs
drivers from official NVIDIA repositories. Driver installation supports Ubuntu, Debian,
RHEL, AlmaLinux, Rocky Linux, Oracle Linux, Fedora, Amazon Linux, Azure Linux,
openSUSE, SLES, and KylinOS. WSL is intentionally rejected because its NVIDIA
driver must be installed on the Windows host.

Repository targets are resolved from the exact distribution, release, and CPU
architecture. If NVIDIA does not publish that exact target, `cudaenv` stops
instead of borrowing another distribution's repository.

## Install

```bash
curl -LsSf https://raw.githubusercontent.com/chengpong1127/cudaenv/main/install.sh | sh
```

The installer puts `cudaenv` in `~/.local/bin` and then asks whether to install
your CUDA environment. The guided setup lets you choose the NVIDIA driver only
or the driver plus CUDA Toolkit, shows the complete plan, and asks for
confirmation before changing the system.

To install the binary without starting CUDA setup, answer `n` at the prompt and
run `cudaenv install` later. Set `CUDAENV_INSTALL_DIR` to use a different binary
directory.

## Build and test

```bash
cargo build
cargo test
```

## Guided installation

```bash
cargo run -- install
cargo run -- install --profile model-training
cargo run -- install --profile cuda-development
cargo run -- install --toolkit 13.1
cargo run -- install --profile cuda-development --dry-run
```

With no `--profile`, `install` asks directly whether to install the NVIDIA
driver only or the driver plus NVIDIA's latest stable CUDA Toolkit. The existing
profile flag names remain available for scripts.

Driver flavor selection is automatic for recognized GPUs. If the GPU generation
cannot be identified safely, cudaenv stops and asks for an explicit
`--driver open` or `--driver proprietary` choice.

Every install prints the full repository and package command plan first. It asks
for confirmation unless `--yes` is supplied; `--dry-run` never changes the
system. For CUDA development, the unversioned `cuda-toolkit` meta-package tracks
the latest stable toolkit in NVIDIA's repository. An optional pin such as
`--toolkit 13.1` selects `cuda-toolkit-13-1` and implies the CUDA development
profile. The network repository is configured only when needed, package
availability is checked before installation, and `nvcc --version` verifies the
result.

Other inspection commands remain available:

```bash
cargo run -- status
cargo run -- doctor
cargo run -- uninstall
cargo run -- uninstall --yes
```

`status` reports driver package installation separately from the loaded driver
runtime, as well as the active CUDA Toolkit version. Install plans query the
system package database so already-installed components are skipped. On Ubuntu,
uninstall resolves and displays the exact installed meta-packages it will remove;
it retains dependencies instead of running a broad automatic cleanup.

Release downloads are verified against SHA-256 checksum files published with
each release.

## Architecture

The crate separates shared workflows from vendor integrations:

```text
src/
├── commands/       CLI workflows and confirmation
├── model/          vendor-neutral devices, status, diagnostics, and plans
├── platform/       operating-system, process, and package-manager adapters
├── providers/
│   └── nvidia/     NVIDIA detection, driver policy, repositories, and CUDA
└── ui/             terminal rendering and prompts
```

Inspection commands use the `AcceleratorProvider` contract. Adding an AMD or
Intel integration starts with a sibling module under `providers/` that returns
the shared `GpuDevice`, `ProviderStatus`, and `Diagnostics` models, followed by
registration in `providers::registered`. Vendor-specific installation options
remain inside the provider because CUDA, ROCm, and oneAPI do not have identical
driver or toolkit semantics.

Installation and removal are represented as typed `OperationPlan` values. The
terminal prints the same `CommandSpec` values that the command runner executes,
so previews cannot silently diverge from the requested system changes.
