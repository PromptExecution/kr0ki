# Design note: durable A2A over NATS

Status: proposed migration contract

## Decision

Use NATS JetStream as the durable internal transport for A2A-shaped agent
coordination. Keep the A2A concepts that make coordination interoperable—Agent
Card, task and context IDs, lifecycle states, correlation IDs, retries, and
separate transport/agent acknowledgements—but do not pretend that NATS subjects
alone are the HTTP A2A protocol. An HTTP Agent Card and edge adapter can be added
when an external boundary is needed.

The existing Redis `b00t agent` path remains a compatibility bridge during the
migration. New features target the NATS contract.

## Durable contract

The `B00T_A2A` stream captures `a2a.v1.>` and retains registration, task events,
acknowledgements, infrastructure squawks, and raw audit traffic according to the
operator's retention policy. The minimum subjects are:

```text
a2a.v1.registry.register
a2a.v1.registry.heartbeat
a2a.v1.registry.leave
a2a.v1.msg.<agent_id>
a2a.v1.task.<task_id>.events
a2a.v1.ack.<correlation_id>
a2a.v1.squawk.infra
a2a.v1.audit.>
```

Every envelope has a unique `event_id`, `correlation_id`, `agent_id`,
`protocol_version`, UTC timestamp, and a payload type. Task envelopes also carry
`task_id` and `context_id`; retries preserve those IDs and change only the delivery
attempt. Registration publishes an Agent Card containing endpoint/subjects and
capabilities, followed by heartbeats with a bounded TTL.

Infrastructure mutation events use this shape conceptually:

```json
{
  "event_id": "uuid",
  "correlation_id": "uuid",
  "agent_id": "sm3lly-acp",
  "host": "fung1",
  "phase": "START",
  "action": "kubectl apply",
  "scope": "kr0ki",
  "reason": "deploy private render service",
  "approval": "issue-804",
  "repo": "PromptExecution/kr0ki",
  "git_sha": "...",
  "recipe": "just pod-up",
  "timestamp": "2026-09-23T00:00:00Z",
  "result": "redacted"
}
```

Publish with a JetStream acknowledgement and `Nats-Msg-Id` set to `event_id`.
Consumers use durable names, explicit ACKs, `AckWait`, and bounded `MaxDeliver`.
Handlers must be idempotent. The sender records both the publish ACK and the
agent/task ACK; these are different facts.

## Rollout

1. **Probe** — register agents and publish signed/redacted squawks while retaining
   the existing Redis path. Acceptance: a remote agent can be discovered, a
   durable event survives subscriber restart, and duplicate delivery is harmless.
2. **Bridge** — implement a b00t adapter translating Redis agent messages to the
   NATS envelope and back. Acceptance: one task can complete through either path
   with the same task/context/correlation IDs and no duplicate side effect.
3. **Measure** — record publish latency, ACK latency, redeliveries, stale leases,
   and task completion/failure. Preserve raw traffic for emergence analysis with
   secrets removed.
4. **Decide** — make NATS the default for new A2A work only after the bridge is
   proven on sm3lly/fung1 and the common infrastructure squawk channel is visible
   to the historian. Retire Redis only after all registered agents have migrated.

## Non-goals

- Replacing the renderer or making kr0ki host an agent model layer.
- Sending credentials or unrestricted shell transcripts through NATS.
- Treating a broker publish ACK as proof that a remote agent acted.
- Introducing a public `kroki.io` dependency; rendering remains air-gapped.
- Requiring every internal NATS message to be directly consumable as HTTP A2A.

Issue #804 is the coordination reference for the broader A2A edge-adapter
proposal; this note defines kr0ki's local operational contract.
