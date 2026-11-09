# ADR 0001: Rust and Tokio Runtime

- Status: accepted
- Date: 2026-11-01

## Context

The core must run on Linux, macOS and Windows, remain independent of UI and model SDKs, support cancellation and timeouts, and eventually serve CLI, desktop and terminal hosts.

## Decision

Use Rust 2024 for the domain/runtime core and Tokio for asynchronous I/O and cancellation. Keep the first host as a synchronous CLI over the async application service.

## Alternatives

- Python: fast iteration, but packaging and long-term desktop embedding are weaker for this goal.
- TypeScript: strong host/UI ecosystem, but fewer guarantees around concurrency and domain boundaries without a much larger dependency surface.
- Go: portable and simple, but not installed in the current build environment.

## Consequences

- Strong typed contracts and a portable binary.
- More upfront code for JSON protocol and provider adaptation.
- Real-model providers remain replaceable and cannot own the agent loop.
