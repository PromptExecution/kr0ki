# Ledgrrr state-machine contract registry boundary

## Decision

kr0ki remains stateless with respect to procedural state machines. It may name
the contract that governed a render request, but it neither stores contract
definitions nor executes transitions. Ledgrrr owns the contract registry as a
database plugin or managed service capability.

PostgreSQL is deliberately an implementation choice below that boundary. A
Hive-managed PostgreSQL service, a self-hosted air-gapped PostgreSQL server,
and a local developer instance must present the same contract to their
clients. kr0ki must not require a particular provider, an extension, a proxy,
or direct SQL access.

## Minimal local `demo-basic`

`just pod-up` is the demo-basic footprint: kr0ki, its renderer, and the local
HTTP UX on `http://localhost:8787`. No PostgreSQL is started. The existing
in-process HTTP tests remain the proof that this path works without network or
database services.

Model rendering is opt-in. A developer supplies `KR0KI_SYSMLV2_BASE_URL` in
the gitignored `.env`; kr0ki then reads a SysML v2 model snapshot through that
HTTP API. The model server's PostgreSQL connection and lifecycle are outside
kr0ki's deployment.

## Future Ledgrrr contract

The only cross-service shape needed now is a stable contract reference:

```json
{
  "contract": "ledgrrr://state-machines/sysml-render/v1",
  "subject": "kr0ki.b00t.promptexecution.com",
  "operation": "render.snapshot",
  "request_id": "opaque-correlation-id"
}
```

Ledgrrr defines the transition vocabulary, validation semantics, retention,
and attributed-usage record. Hive provides discovery, credentials, tenancy,
and service lifecycle. kr0ki can attach this reference to a request or log it
as metadata once that public contract exists; until then it has no Ledgrrr or
PostgreSQL runtime dependency.

## Non-goals

- No Rust PostgreSQL extension in kr0ki.
- No SQL parsing or billing inside a PGWire proxy.
- No PostgreSQL sidecar in the default kr0ki Pod.
- No replacement state-machine implementation competing with Ledgrrr.

<!-- b00t:map v1
summary: provider-neutral boundary between kr0ki and Ledgrrr state-machine registry
tags: kr0ki, ledgrrr, postgres, pgwire, state-machine, air-gap
tier: frontier
cmds: just pod-up, just test
complexity: 3
-->
