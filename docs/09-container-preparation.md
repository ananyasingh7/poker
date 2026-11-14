# Deferred Docker/OCI deployment notes

**Decision:** implement native Rust subprocess supervision first. The authoritative implementation plan is [Milestone 5: Native Rust subprocess participants](../milestones/05-native-subprocesses.md). Docker/OCI is a later optional deployment backend and is not required for milestone 5 acceptance.

## Preserve the transport boundary

The referee owns the deck, legality, money, session control, and global budget. Participants receive only their authorized observations and return actions or lifecycle acknowledgements. The existing development v1 types lack startup, hand-context and usage messages; the native-process plan introduces v2 before any production container contract is frozen. Reuse that completed contract when implementing a container launcher.

The current six-model table contains six independent API contexts in one Rust process. A future container would package a small Rust worker calling OpenRouter; it need not contain model weights or run a separate local model instance.

## Future Docker/OCI work

- Package the Rust worker in a multi-stage Linux image; confirm target architecture and installed runtime before choosing build targets.
- Run non-root, with a read-only root filesystem, limited tmpfs, dropped capabilities, no privileged mode, and explicit CPU/memory/PID limits. Never mount the host container socket, repository, evaluator logs, or another seat's private data.
- Scripted agents need no network. Restrict model-agent egress to the inference service through the selected runtime/proxy. Inject credentials through a private runtime secret or host-owned inference proxy; never bake them into images.
- Extend the native supervisor's deadlines, termination, usage reconciliation, and crash tests to container lifecycle management. Test filesystem/network restrictions and resource exhaustion directly before claiming isolation.
- Compare complete scripted and model sessions against the native backend, measuring actual startup time, latency, resource usage, and cost.

Containers share a kernel and are not full VMs or an absolute hostile-code boundary. Native processes running as the same OS user likewise cannot be treated as hostile-code sandboxes. Consider separate VMs/microVMs only when the threat model warrants the additional operational cost.

## Local readiness observation

On 2026-11-11, the Docker CLI was installed at `/usr/local/bin/docker`, but `docker version` could not connect to the configured daemon. No runtime was started and no container execution was verified. This does not block native subprocess work.

## Later model-hosting choices

Keep hosted OpenRouter calls as the first deployment. A shared local inference server or independent local model copies are separate experimental options requiring actual memory, throughput and latency measurements. Linux containers on macOS do not imply native Apple GPU access. Firecracker requires Linux KVM and is not a direct native macOS replacement.

References: [Docker namespaces/security](https://docs.docker.com/engine/security/), [Docker resource limits](https://docs.docker.com/engine/containers/resource_constraints/), and [Firecracker requirements](https://github.com/firecracker-microvm/firecracker/blob/main/docs/getting-started.md). Docker containers have no CPU/memory ceiling by default; resource restrictions must be configured explicitly.

## Claude feedback (2026-11-11)

- **Native supervision is done; containers now add only isolation.** Deadlines, kill/reap, budget reconciliation, private per-seat directories and crash tests all exist ([details](10-native-subprocess-supervision.md)). A container backend would reuse protocol v2 unchanged; the launcher would swap the executable for a `docker run -i` style command.
- **I would not build it yet.** Every worker is project-owned code. Containers earn their cost when third-party agents arrive or when you need a hard network boundary. Until then they add a daemon dependency and startup time (native startup for six workers is about 10 ms).
- **Build the inference proxy first.** A small host-owned proxy that holds the API key and enforces the request and dollar caps before forwarding would (a) remove the credential from workers, (b) make the budget a real pre-request limit instead of self-reported usage, and (c) be the single egress point a container network policy would later allow. It is useful with or without containers.
- **Cheaper isolation steps exist.** Running workers under a separate OS user, or under `sandbox-exec` on macOS and Landlock/seccomp on Linux, gives filesystem and network restrictions without images. Worth trying before Docker if the threat model grows.
- **One thing to carry over.** Workers exit on stdin end-of-file. Keep that in any container entry point so orphaned containers cannot keep spending.
