# AI-governance crosswalk for kr0ki requirement tags (as of 2026-10-01)

Research aid, not legal advice. Verification key: [V] = confirmed this session from a fetched/searched source; [K] = from my prior knowledge of the published text, not re-fetched; UNVERIFIED = do not tag until checked. I did not reach ISO's paywalled text, so Annex A titles beyond what was confirmed are [K].

## 1. EU AI Act - Regulation (EU) 2024/1689 (EUR-Lex; Commission AI Act Service Desk)
- Tiers: unacceptable (Art. 5 prohibitions), high-risk (Art. 6, Annex I product-embedded, Annex III standalone), limited/transparency (Art. 50), minimal; plus GPAI models (Arts. 51-56) with a systemic-risk subset. [K]
- High-risk requirements: Art. 9 risk management system; Art. 10 data and data governance; Art. 11 technical documentation; Art. 12 record-keeping; Art. 13 transparency and information to deployers; Art. 14 human oversight; Art. 15 accuracy, robustness, cybersecurity. [K]
- Providers: Art. 16 obligations; Art. 17 quality management system. Deployers: Art. 26; Art. 27 fundamental-rights impact assessment; Art. 72 post-market monitoring; Art. 73 serious-incident reporting. [K]
- Dates: in force 1 Aug 2024; prohibitions and AI literacy 2 Feb 2025; GPAI 2 Aug 2025 [K/V]. **Changed recently:** the Digital Omnibus on AI, Regulation (EU) 2026/1744, was published in the OJ 24 Jul 2026 and entered into force 27 Jul 2026 [V, secondary source: ertico.com - confirm on EUR-Lex]. It defers high-risk obligations to **2 Dec 2027 (Annex III)** and **2 Aug 2028 (Annex I)**, sandboxes to 2 Aug 2027, and gives a watermarking (Art. 50(2)) grace to 2 Dec 2026 for existing systems. Art. 50 transparency otherwise applied from 2 Aug 2026. New Art. 5 ban on non-consensual intimate imagery/CSAM generation (transition to 2 Dec 2026) [V: Gibson Dunn, provisional agreement 7 May 2026].
- URLs: https://eur-lex.europa.eu/eli/reg/2024/1689/oj ; https://digital-strategy.ec.europa.eu/en/policies/regulatory-framework-ai

## 2. Australia
- **Voluntary AI Safety Standard (VAISS)**, Dept of Industry, Science and Resources (DISR), Sept 2024. 10 guardrails [K]: G1 accountability process/governance/strategy; G2 risk management process; G3 protect AI systems, data governance and data quality; G4 test, evaluate and monitor; G5 human control/oversight; G6 inform end-users about AI decisions/interactions; G7 processes for people to challenge use/outcomes; G8 transparency across the AI supply chain; G9 keep records/documentation for third-party assessment; G10 engage stakeholders (safety, diversity, fairness).
- **Guidance for AI Adoption** (NAIC/DISR), published 21 Oct 2025; first update to VAISS; consolidates 10 guardrails into **6 essential practices** [V]: (1) decide who is accountable; (2) understand impacts and plan accordingly; (3) measure and manage risks; (4) share information; (5) test and monitor; (6) maintain human control. Two parts: Foundations and Implementation Practices. A NAIC crosswalk from the 10 guardrails to the 6 practices exists (reported by a secondary source; the document itself not retrieved) - fetch it from industry.gov.au/naic before relying on my mapping. Claimed alignment with ISO/IEC 42001 and NIST AI RMF [V, secondary].
- **Mandatory guardrails**: proposals paper (Sept 2024) for high-risk AI was **abandoned** with the National AI Plan, 2 Dec 2025, in favour of adapting existing law [V, secondary: Piper Alderman et al.].
- APS: DTA/Finance "Policy for the responsible use of AI in government" (v2.0 in force Dec 2025 - UNVERIFIED version/date; not fetched). Relevant only if the customer is a federal agency.
- URLs: https://www.industry.gov.au/publications/guidance-for-ai-adoption ; https://www.industry.gov.au/publications/voluntary-ai-safety-standard

## 3. ISO/IEC 42001:2023 (AI management system; ISO/IEC JTC 1/SC 42; https://www.iso.org/standard/81230.html)
- Clauses (Harmonised Structure, same as ISO 27001): 4 Context; 5 Leadership; 6 Planning (risk/opportunity, AI risk assessment, AI system impact assessment, objectives); 7 Support; 8 Operation; 9 Performance evaluation; 10 Improvement. [K]
- Annex A groups [K; A.2.2-A.2.4 and A.4.2-A.4.6 id existence seen in search]: A.2 Policies related to AI (A.2.2 AI policy, A.2.3 alignment with other policies, A.2.4 review); A.3 Internal organization (A.3.2 roles/responsibilities, A.3.3 reporting concerns); A.4 Resources for AI systems (A.4.2 resource documentation, A.4.3 data, A.4.4 tooling, A.4.5 system/computing, A.4.6 human); A.5 Assessing impacts of AI systems (A.5.2 process, A.5.3 documentation, A.5.4 individuals/groups, A.5.5 societal); A.6 AI system life cycle (A.6.1.x responsible development; A.6.2.2-A.6.2.8 requirements, design, V&V, deployment, operation/monitoring, technical documentation, event logs); A.7 Data for AI systems (A.7.2-A.7.6 development, acquisition, quality, provenance, preparation); A.8 Information for interested parties (A.8.2-A.8.5 user documentation, external reporting, incident communication, information sharing); A.9 Use of AI systems (A.9.2-A.9.4 responsible-use processes, objectives, intended use); A.10 Third-party and customer relationships (A.10.2-A.10.4 responsibilities, suppliers, customers). Treat sub-control titles as paraphrase; verify against the licensed text before quoting.
- Relations: integrates with ISO/IEC 27001 (shared clause structure); ISO/IEC 23894:2023 gives AI risk-management guidance supporting clause 6.

## 4. NIST AI RMF 1.0 (NIST AI 100-1, Jan 2023, https://doi.org/10.6028/NIST.AI.100-1; AIRC https://airc.nist.gov)
- Four functions: GOVERN, MAP, MEASURE, MANAGE. Categories numbered per function (GOVERN 1-6, MAP 1-5, MEASURE 1-4, MANAGE 1-4) and subcategories as `GOVERN 1.1` [K]. Playbook gives suggested actions.
- Generative AI Profile: NIST AI 600-1, July 2024 (12 GenAI-specific risks) [K].
- AIRC hosts crosswalks (e.g. to ISO/IEC 23894); a NIST-published ISO 42001 crosswalk: UNVERIFIED.

## 5. AESCSF and CIRMP
- **AESCSF** (Australian Energy Sector Cyber Security Framework): run by AEMO with DCCEEW/Home Affairs (CISC) and industry; built on US DOE C2M2 and NIST CSF [V]. Version 2 (2022) added practices and domain changes; annual assessment cycles continue (2023, 2024, 2025 overview published) [V]. Roughly 11 domains (ID/PM... e.g. risk, asset/change/config, threat/vulnerability, identity/access, situational awareness, event/incident response, supply chain, workforce, architecture, program management); the exact 2025 domain count and ids: UNVERIFIED. Maturity Indicator Levels MIL-1..3; Security Profiles SP-1..3 by criticality [V]. Voluntary self-assessment (plus AESCSF Lite).
- **CIRMP** (SOCI Act 2018, Part 2A; CIRMP Rules 2023): responsible entities of specified critical infrastructure assets (electricity, gas, water, etc.) must adopt and comply with a written program covering four hazards: cyber/information security, personnel, supply chain, physical/natural [V]. AESCSF (at specified security profile/MIL) is one of the prescribed frameworks for the cyber hazard, alongside others (NIST CSF, ISO 27001, Essential Eight, 2023-24 C2M2); the entity's board must attest annually. **Changed recently:** Enhanced CIRMP Rules registered 9 Jun 2026, adding foreign ownership/control/influence (FOCI) risk, phishing-resistant MFA, network segregation, AusCheck/NV1 vetting of critical workers, supply chain mapping and sanctions screening, with 12-24 month grace periods; higher maturity levels and latest ISO 27001 [V, single secondary source: Clayton Utz - confirm on cisc.gov.au].
- AI relevance: AI embedded in energy OT/IT is part of the asset's cyber and supply-chain hazards; there is no AI-specific CIRMP control. Mapping is by analyst judgement.

## 6. Crosswalk (Australian guardrails -> others)
Basis for all rows: **analyst judgement** unless noted. No official, retrieved crosswalk covers these links (the NAIC guardrail->practice crosswalk is official but not retrieved; VAISS/Guidance claim ISO 42001 and NIST alignment generally).

| AU guardrail (practice) | ISO 42001 | NIST AI RMF | EU AI Act | AESCSF/CIRMP | Basis |
|---|---|---|---|---|---|
| G1 Accountability (P1) | cl.5, A.2, A.3 | GOVERN 1, 2 | Art. 17, 16 | CIRMP board attestation; AESCSF program mgmt | analyst judgement |
| G2 Risk management (P3) | cl.6.1, A.5 | MAP 1-5, MANAGE 1-2 | Art. 9, 27 | CIRMP all four hazards | analyst judgement |
| G3 Data/system protection (P3,P5) | A.7, A.4 | MEASURE 2, MAP 4 | Art. 10, 15 | CIRMP cyber; AESCSF identity/access, threat | analyst judgement |
| G4 Test/monitor (P5) | A.6.2.4-A.6.2.6 | MEASURE 1-4, MANAGE 4 | Art. 15, 72, 73 | AESCSF event/incident response | analyst judgement |
| G5 Human control (P6) | A.9, A.6.2.5 | GOVERN 3, MANAGE 2 | Art. 14, 26 | OT safety/operator override; CIRMP physical | analyst judgement |
| G6 Inform end-users (P4) | A.8 | GOVERN 5, MAP 3 | Art. 13, 50 | n/a | analyst judgement |
| G7 Contestability (P2/P4) | A.8.3, A.3.3 | GOVERN 5, MANAGE 4 | Art. 86 (right to explanation), 27 | n/a | analyst judgement |
| G8 Supply-chain transparency (P4) | A.10 | GOVERN 6 | Art. 25, 53 | CIRMP supply chain (+FOCI 2026); AESCSF supply chain | analyst judgement |
| G9 Records (P4) | A.6.2.7-A.6.2.8, cl.7.5 | GOVERN 1.6 (inventory) | Art. 11, 12, 18 | CIRMP written program/records | analyst judgement |
| G10 Stakeholder engagement (P2) | A.5.4, cl.4.2 | MAP 5, GOVERN 5 | Art. 27, 4 (AI literacy) | CIRMP personnel | analyst judgement |

Subcategory ids such as GOVERN 1.6 are [K]; use category-level tags unless verified against the AI RMF Playbook.

## 7. How a requirement should carry these tags
Syntax: `framework:control-id`, lowercase framework slug, control id verbatim, spaces replaced by `-`. Multiple tags per requirement; add `#official` or `#judgement` suffix for provenance and `@verified-YYYY-MM-DD`.
- `au-vaiss:G3`, `au-ai6:P5`, `iso42001:A.6.2.6`, `iso42001:cl.6.1`, `nist-ai-rmf:GOVERN-1.1`, `nist-ai-600-1:GAI-risk` (id UNVERIFIED), `eu-ai-act:art-9`, `cirmp:supply-chain`, `aescsf:SP-2/MIL-2`.
- Store tag registry with: version, source URL, applicability date, status (e.g. `eu-ai-act:art-9` applies 2027-12-02 for Annex III after omnibus).

Starter coverage questions:
1. Which requirements have no tag in at least one of the six AI6 practices?
2. For each high-risk (EU Annex III) function, are all of Art. 9-15 covered by a verified requirement?
3. Which ISO 42001 Annex A groups (A.2-A.10) have zero requirements, and is a Statement of Applicability exclusion justified?
4. Which AI components in energy/critical-infrastructure assets lack a CIRMP hazard tag (cyber/personnel/supply-chain/physical) and an AESCSF target SP/MIL?
5. Which tags rest on `#judgement` mappings rather than official crosswalks, and which are past their `@verified` date?
