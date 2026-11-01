# ADR 0003: OpenAI-compatible Provider Adapter

- Status: accepted
- Date: 2026-11-01

## Context

DeepSeek, Kimi, Qwen, Ollama and several gateways expose OpenAI-compatible chat completion APIs. The agent must not adopt that wire format as its internal contract.

## Decision

Implement one narrow OpenAI-compatible adapter for the first real-provider proof. Translate canonical messages, tool calls and errors at the adapter boundary. Keep provider-specific fields out of domain types.

## Consequences

- One adapter can validate several providers with different base URLs.
- Compatibility does not imply capability parity; provider manifests and live tests remain required.
- Provider-specific extensions such as reasoning fields, caching and streaming require separate adapters or capabilities later.
