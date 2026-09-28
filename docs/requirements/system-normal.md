# System-Normal Requirements

**System:** kr0ki (System-Normal)  
**Version:** 0.0.1  
**Last Updated:** 2026-09-26  
**Status:** Active

---

## Overview

System-Normal refers to the core kr0ki rendering service and its essential capabilities. This document defines the foundational requirements for the diagram-as-code rendering pipeline.

---

## Functional Requirements

### FR-001: Diagram Rendering

**Priority:** Must  
**Status:** Implemented

The system shall accept diagram source code in supported formats and render them as SVG or PNG images.

**Acceptance Criteria:**
- [x] System accepts D2, GraphViz, PlantUML, Mermaid, and other supported formats
- [x] System returns rendered image in requested format (SVG/PNG)
- [x] System validates input syntax before rendering
- [x] System returns appropriate error messages for invalid input

---

### FR-002: Content-Addressed Caching

**Priority:** Must  
**Status:** Implemented

The system shall cache rendered diagrams using content-addressed storage to avoid redundant rendering.

**Acceptance Criteria:**
- [x] Cache key is derived from SHA-256 hash of source content
- [x] Cache returns identical output for identical input
- [x] Cache supports both SVG and PNG output variants
- [x] Cache is persistent across service restarts

---

### FR-003: Health and Status Reporting

**Priority:** Must  
**Status:** Implemented

The system shall expose health and status endpoints for monitoring and orchestration.

**Acceptance Criteria:**
- [x] `/health` endpoint returns service status
- [x] Health check includes dependency status (backend, cache)
- [x] Response includes version information
- [x] Health endpoint responds within 100ms

---

### FR-004: Contract Reference Boundary

**Priority:** Should  
**Status:** Implemented

The system shall attach contract reference metadata to all responses for future Ledgrrr integration.

**Acceptance Criteria:**
- [x] All responses include `X-Kr0ki-Contract` header
- [x] All responses include `X-Kr0ki-Request-Id` header
- [x] Request ID is UUID v7 or adopted from caller
- [x] Contract URI is configurable via environment variable

---

## Non-Functional Requirements

### NFR-001: Performance

**Priority:** Must

The system shall render diagrams within acceptable time limits.

**Acceptance Criteria:**
- [ ] Simple diagrams (< 50 nodes) render in < 1 second
- [ ] Complex diagrams (< 200 nodes) render in < 5 seconds
- [ ] Cache hits return in < 100ms
- [ ] System handles 100 concurrent requests without degradation

---

### NFR-002: Reliability

**Priority:** Must

The system shall maintain high availability and graceful degradation.

**Acceptance Criteria:**
- [ ] Service uptime > 99.5% (excluding maintenance windows)
- [ ] Backend failures return 503 with retry guidance
- [ ] Cache failures fall back to direct rendering
- [ ] Service restarts within 10 seconds

---

### NFR-003: Resource Usage

**Priority:** Should

The system shall operate within defined resource constraints.

**Acceptance Criteria:**
- [ ] Memory usage < 512MB per render operation
- [ ] CPU usage < 1 core for single render
- [ ] Cache storage < 10GB for typical workload
- [ ] No memory leaks over 24-hour operation

---

## Interface Requirements

### IR-001: HTTP API

**Priority:** Must  
**Status:** Implemented

The system shall expose a RESTful HTTP API for diagram rendering.

**Acceptance Criteria:**
- [x] POST `/render/{format}` accepts diagram source
- [x] GET `/health` returns service status
- [x] GET `/formats` lists supported formats
- [x] API follows REST conventions and returns appropriate status codes

---

### IR-002: Kroki Backend Integration

**Priority:** Must  
**Status:** Implemented

The system shall integrate with a Kroki-compatible backend for rendering.

**Acceptance Criteria:**
- [x] Backend URL is configurable via environment variable
- [x] System handles backend unavailability gracefully
- [x] System supports backend health checking
- [x] System can switch between backends without restart

---

## Constraint Requirements

### CR-001: Stateless Operation

**Priority:** Must

The system shall operate without persistent state beyond the render cache.

**Acceptance Criteria:**
- [x] No database dependencies
- [x] No session state between requests
- [x] All configuration via environment variables
- [x] Service can be scaled horizontally

---

### CR-002: Security

**Priority:** Must

The system shall enforce security boundaries for rendering operations.

**Acceptance Criteria:**
- [x] Input validation prevents injection attacks
- [x] Resource limits prevent denial-of-service
- [x] Authentication supported via bearer token
- [x] No arbitrary code execution in render pipeline

---

## Traceability Matrix

| Requirement | Design | Implementation | Test | Status |
|-------------|--------|----------------|------|--------|
| FR-001 | PRD-KR0KI-001 | kr0ki-core | test_render | ✅ |
| FR-002 | PRD-KR0KI-001 | kr0ki-core::cache | test_cache | ✅ |
| FR-003 | PRD-KR0KI-001 | kr0ki-server | test_health | ✅ |
| FR-004 | Issue #57 | kr0ki-server::contract | test_contract | ✅ |
| NFR-001 | PRD-KR0KI-001 | kr0ki-core | perf_test | ⚠️ |
| NFR-002 | PRD-KR0KI-001 | kr0ki-server | test_health | ⚠️ |
| NFR-003 | PRD-KR0KI-001 | kr0ki-core | resource_test | ⚠️ |
| IR-001 | PRD-KR0KI-001 | kr0ki-server | test_http | ✅ |
| IR-002 | PRD-KR0KI-001 | kr0ki-core::render | test_backend | ✅ |
| CR-001 | PRD-KR0KI-001 | kr0ki-server | test_stateless | ✅ |
| CR-002 | PRD-KR0KI-001 | kr0ki-server | test_security | ⚠️ |

---

## Change Log

| Date | Version | Change | Author |
|------|---------|--------|--------|
| 2026-09-26 | 1.0 | Initial requirements baseline | kr0ki team |
