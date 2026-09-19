import { describe, expect, it } from "vitest";
import { AuiProvider, AuiConfig, useAui, useAuiState, useAuiEvent } from "@assistant-ui/vue";

// Phase 0 spike gate (Plan 004 / ADR-0001): the vendored b00t
// @assistant-ui/vue package must be importable and expose the contract
// surface the workspace shell will build on.
describe("vendored @assistant-ui/vue", () => {
  it("exports the provider and config factories", () => {
    expect(typeof AuiProvider).toBe("object"); // Vue component
    expect(typeof AuiConfig).toBe("function");
  });

  it("exports the composables used by the workspace shell", () => {
    expect(typeof useAui).toBe("function");
    expect(typeof useAuiState).toBe("function");
    expect(typeof useAuiEvent).toBe("function");
  });
});
