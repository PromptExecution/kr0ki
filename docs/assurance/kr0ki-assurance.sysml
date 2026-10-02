// Runtime of the kr0ki requirements agent ("reqiffy"), modelled as parts.
//
// Enforcement of identity and sandboxing lives in their deployment components
// (WorkloadIdentity, ExecutionSandbox); kr0ki displays their relationships and
// evidence, it does not implement them. Qualified names here are the locators
// that `system_element` nodes in kr0ki.assurance.toml resolve against (KR-A02).
package KrOKiAssurance {

    // The agent process that proposes and requests changes. Untrusted by design.
    part def AgentWorkload;

    // Who the workload is, and what it may do (its capability grant).
    part def WorkloadIdentity;

    // The single path from agent to tools; enforces the grant on every transport.
    part def ToolGateway {
        part audit : AuditRecorder;
    }

    // Appends one record per tool invocation, permitted or denied.
    part def AuditRecorder;

    // Confines the workload's filesystem writes to declared mounts.
    part def ExecutionSandbox;

    // Serves revision-qualified model and requirement content; accepts commits.
    part def ModelService;

    // Runs declared verification cases and returns evidence references.
    part def VerificationRunner;

    // Holds revision-bound evidence records and their artifacts.
    part def EvidenceStore;

    part def AssuranceRuntime {
        part workload : AgentWorkload;
        part identity : WorkloadIdentity;
        part gateway : ToolGateway;
        part sandbox : ExecutionSandbox;
        part modelService : ModelService;
        part runner : VerificationRunner;
        part evidenceStore : EvidenceStore;
    }
}
