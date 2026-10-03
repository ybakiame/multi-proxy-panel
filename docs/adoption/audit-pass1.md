# Pass 1 — Protocol Adoption Audit (ProxyPanel)

- **Status:** Pass 1 complete — audit only. No Pass 2 work was started.
- **Audited revision:** `1bf847a30fd6436bf4e2f632d37d3e09a308ec30`
- **Working tree at audit time:** dirty — `AGENTS.md` modified (+14 lines, "Agent Development Protocol" section, working tree only) and root `ADOPT.md` untracked.
- **Protocol submodule:** `.protocol/agent-development-protocol` pinned to `310fba5baa384c912e1fd8300e33d72a057908c5`, protocol version **0.1.0** (`protocol-manifest.json:2`), submodule working tree clean.
- **Audit date:** 2026-10-04
- **Artifacts produced:** `docs/adoption/project-inventory.json`, `docs/adoption/protocol-mapping.json`, this document.

---

## 1. Method and authority boundaries

Discovery was read-only and broad-to-narrow per the Protocol's Discovery phase: repository shape → build/runtime → validation → conventions → agent-specific components → existing documentation and decisions.

Authority used for this audit:

1. ProxyPanel project authority: `AGENTS.md`, `README.md`, `docs/`, existing ADRs, source/configuration/CI.
2. Protocol authority: `.protocol/agent-development-protocol/` (manifest, adoption docs, conformance doc, schemas) — used to define the capability vocabulary and the audit workflow only.
3. Protocol files are **not** treated as ProxyPanel project documentation, and no Protocol file was copied into the project.

Classification follows the Protocol test (`mapping.md:18-32`): applicability first, then substantive material, then protocol-intent satisfaction. `recommended_paths` from `protocol-manifest.json` were used only as guidance — a missing recommended directory was never treated as `Missing`.

Evidence quality order followed `discovery.md:87-96`: implementation and configuration first, then tests/CI, then explicit decisions (ADRs), then documentation, then structure/naming, then inference. Every `Existing`/`Partial`/`Missing` classification in `protocol-mapping.json` carries concrete evidence; every `N/A` carries an explicit applicability reason.

---

## 2. Protocol files vs ProxyPanel project files

Explicitly distinguished during this audit:

| Kind | Paths |
|---|---|
| Protocol reference material (submodule, read-only) | `.protocol/agent-development-protocol/**` (`ADOPT.md`, `protocol-manifest.json`, `docs/**`, `schemas/**`, `CHANGELOG.md`, `README.md`, `AGENTS.md`) |
| Protocol-cited but project-owned | root `ADOPT.md` (project launcher, untracked), `AGENTS.md:7-19` (project's protocol consumption rules) |
| ProxyPanel project authority | `AGENTS.md`, `README.md`, `docs/**`, `docs/adr/**`, `Cargo.toml`, `package.json`, `crates/**`, `apps/**`, `packages/**`, `.github/workflows/**`, `scripts/**`, `.agents/**`, `.opencode/**`, `.pi/**`, `opencode.jsonc` |

`.protocol/agent-development-protocol/AGENTS.md` and `.opencode/AGENTS.md` are submodule / vendor files, respectively; they are **not** ProxyPanel's root project contract.

---

## 3. Discovery coverage

| Area | Sources inspected |
|---|---|
| Repository shape | root listing, `crates/` (13 workspace members), `apps/` (client, panel), `packages/client-core`, `proto/`, `scripts/`, `deploy/`, `docs/` |
| Build/runtime | `Cargo.toml`, `package.json`, `bun.lock`, `rust-toolchain.toml`, `scripts/check-rust-gates.sh`, `apps/*/package.json`, `flake.nix` |
| Validation | `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `.husky/pre-commit`, `scripts/check-file-size.sh`, inline `#[cfg(test)]` (83 files), `crates/pp-client/tests/real_core_e2e.rs` |
| Conventions | `AGENTS.md` §3/§5/§9, `docs/contributing.md`, `.agents/rules/*.md` |
| Agent-specific components | `opencode.jsonc`, `.opencode/**`, `.pi/**`, `.agents/rules/**`, `.agents/skills/**`, `skills-lock.json`, `apps/panel/AGENTS.md`, root `AGENTS.md` protocol section |
| Documentation/decisions | `README.md`, `docs/index.md`, `docs/architecture.md`, `docs/development.md`, `docs/deployment.md`, `docs/api_reference.md`, `docs/contributing.md`, `docs/adr/0001-0007`, `docs/plans/`, `docs/research/` |
| Protocol | `.protocol/.../ADOPT.md`, `protocol-manifest.json`, `docs/adoption/*`, `docs/conformance.md`, `schemas/*` |
| Ignored as non-project-source | `node_modules/`, `target/`, `.cache/`, `.opencode/node_modules/`, `.reference/`, `.venv-deploy/`, `dist/` |

---

## 4. Capability classification summary

Vocabulary: `protocol-manifest.json` (13 capabilities). Distribution: **Existing 1 · Partial 6 · Missing 0 · N/A 6**.

| # | Capability | Status | One-line basis |
|---|---|---|---|
| 1 | project-contract | **Partial** | Rich `AGENTS.md`/`README.md`/`docs/index.md` contract, but the contract contains stale repository-shape facts and has no Definition of Done or escalation conditions; the protocol entry point is uncommitted. |
| 2 | architecture | **Partial** | Substantive `docs/architecture.md` (706 lines) grounded in implementation, but the client chapters contradict ADR-0007 and the frontend stack versions contradict `package.json`. |
| 3 | workflows | **Partial** | Commit/PR/review/task recipes exist; protocol-required workflow phases, Definition of Done and stop/escalation conditions are absent. |
| 4 | testing | **Partial** | Rust unit tests, DB/gRPC test guidance and CI gates exist; `AGENTS.md` misstates the integration-test status and no frontend test capability exists. |
| 5 | evaluation | **N/A** | No AI/LLM agent runtime is under development; the only "evaluation" document is an architecture migration assessment. Applicability is argued for the coding-agent infrastructure only (ADC-002). |
| 6 | operations | **Existing** | `docs/deployment.md` + Dockerfiles/compose/install scripts/CLI lifecycle/release CI cover deployment, HA, security, monitoring, backup and troubleshooting. |
| 7 | agents | **N/A** | The product's "Agent" (`pp-agent`) is a proxy-node daemon, not an AI agent; the Protocol capability targets AI Agent architecture (planning/lifecycle/tool use). |
| 8 | tools | **N/A** | No project-owned Agent tool contracts; product interfaces are REST/gRPC/Tauri IPC. |
| 9 | prompts | **N/A** | No application prompt subsystem; coding-agent prompt files are Agent Adapter assets. |
| 10 | memory | **N/A** | No agent memory; in-repo "state" is application state and product persistence. |
| 11 | mcp | **N/A** | No MCP server/client configuration or documentation anywhere in the repository. |
| 12 | adrs | **Partial** | Seven substantive ADRs with a consistent header and a real supersession chain, but no documented ADR convention/template/index and two records have drifted metadata. |
| 13 | agent-adapter | **Partial** | Concrete opencode/pi/agents adapters exist, but there is no adapter documentation, adapter paths are stale, rule sets overlap without a stated precedence, and no adapter conformance evidence exists. |

No capability was classified `Missing`: every capability that applies to this repository already has substantive material. Per `mapping.md:32` and `conformance.md:43-44`, absent recommended paths (e.g. `docs/workflows/`, `docs/testing/`, `docs/decisions/`, `docs/adapters/`) were **not** treated as `Missing`.

---

## 5. Capability detail (evidence, gaps, proposed action)

Full machine-readable evidence is in `docs/adoption/protocol-mapping.json`; this section is the human-readable rationale.

### 5.1 project-contract — Partial

- **Evidence:** `AGENTS.md:1-423`, `README.md:1-422`, `docs/index.md:1-86`, `docs/contributing.md:100-394`, `AGENTS.md:7-19` + `ADOPT.md`.
- **Gaps:** `README.md:235-236` tree lists removed `apps/desktop`/`apps/mobile` while `README.md:70`/`:262` and `apps/` use `apps/client`; no Definition of Done or escalation conditions; the protocol entry point is working-tree-only; `docs/index.md` omits the client and is not linked from the root documents.
- **Action (Pass 2):** reconcile the structure tree and index; add DoD/escalation once; commit the adoption entry point after ADC-004.

### 5.2 architecture — Partial

- **Evidence:** `docs/architecture.md:1-706`, `docs/adr/0007-*.md`, `apps/client/src/{desktop,mobile}`, `apps/client/src-tauri`, `crates/pp-client-tauri`, `packages/client-core`, `proto/*.proto`.
- **Gaps:** `docs/architecture.md:355-362,410,428,432` still describe the pre-merge client; `docs/architecture.md:126`, `docs/development.md:361-362`, `docs/index.md:71` claim React 18/Vite 6 against `apps/panel/package.json` (React ^19.2.7, Vite ^8.2.2); post-merge shell architecture is documented only in `AGENTS.md`; the `AppState` excerpt contradicts `crates/pp-hub/src/state.rs:77-86`.
- **Action (Pass 2):** refresh client chapters citing ADR-0007; fix stack versions; fix or annotate the `AppState` excerpt.

### 5.3 workflows — Partial

- **Evidence:** `AGENTS.md:299-406`, `docs/development.md:102-137,265-356,437-687`, `docs/contributing.md:100-376`, `.husky/pre-commit`, `.github/workflows/ci.yml`, `scripts/check-rust-gates.sh`.
- **Gaps:** no workflow with the Protocol's required phase set; no project Definition of Done (the only DoD-like statements are adapter files: `.opencode/agents/executor.md:34`, `.agents/rules/code-organization.md:64`); no stop/escalation conditions; no per-change-type workflows (feature/bugfix/refactor) or adoption workflow.
- **Action (Pass 2):** add workflow documentation in the agreed location (ADC-001) with all phases, DoD and escalation.

### 5.4 testing — Partial

- **Evidence:** `AGENTS.md:373-381`, `docs/development.md:521-583`, 83 Rust files with `#[cfg(test)]`, `crates/pp-client/tests/real_core_e2e.rs`, `ci.yml:8-120`, `check-rust-gates.sh`, `.husky/pre-commit`.
- **Gaps:** `AGENTS.md:376` says integration tests are not set up while `crates/pp-client/tests/real_core_e2e.rs` exists; `docs/development.md` omits that e2e test and its `--include-ignored` invocation; no frontend tests exist in any app/package (`verify` = build + lint + format).
- **Action (Pass 2):** correct the documented test surface; record the frontend-testing gap (decision deferred to ADC-005).

### 5.5 evaluation — N/A

- **Applicability reason:** the capability targets Agent evaluation (datasets, metrics, regression). ProxyPanel develops no AI/LLM agent application: `crates/pp-agent` manages sing-box/mihomo processes over gRPC, and no LLM dependency exists in the workspace. `docs/plans/2026-10-03-client-merge-evaluation.md` is an architecture migration assessment, not Agent evaluation. No eval artifacts were found in `.github`, `scripts` or `.agents`.
- **Condition:** if ADC-002 brings the coding-agent infrastructure into scope, reclassify to `Missing` and add evaluation coverage.

### 5.6 operations — Existing

- **Evidence:** `docs/deployment.md:24-909` (requirements, Docker Compose, nginx/Caddy, CLI lifecycle, one-click node install, manual systemd, HA, TLS/token security, Prometheus/Grafana monitoring and alerting, backup/restore, upgrade, CI/CD artifacts, troubleshooting), `docker-compose.yml`, `Dockerfile.hub`/`Dockerfile.agent`, `scripts/install-hub.sh`/`install-agent.sh`, `crates/pp-cli` (`InstallTarget::Hub/Agent`), `release.yml`. Default Hub port claim verified (`crates/pp-hub/src/config.rs:35` → `0.0.0.0:8081`).
- **Gaps:** none material; no incident/SLO section (not required for this project's Protocol intent).
- **Action:** keep-alive reconciliation in Pass 2 only.

### 5.7 agents — N/A

- **Applicability reason:** the capability documents the project's own AI Agent architecture. The product's `pp-agent` is a proxy-node daemon (`crates/pp-agent/src/main.rs`, `client.rs`; `proto/hub_agent.proto`), with no planning loop, tool selection or LLM. The `Agent` name must not be read as capability presence. Coding-agent infrastructure (`.agents/`, `.opencode/`, `.pi/`) is Agent Adapter scope.

### 5.8 tools — N/A

- **Applicability reason:** no project-owned Agent Tool input/output/side-effect contracts exist. Product interfaces are HTTP REST, gRPC and Tauri IPC (`docs/api_reference.md`, `proto/`, `crates/pp-client-tauri`); opencode/pi tool lists are vendor adapter configuration.

### 5.9 prompts — N/A

- **Applicability reason:** the capability treats prompts as application behavior of an Agent system. No such system exists; prompt-like files (`.pi/master.md`, `.pi/agents/*.md`, `.opencode/agents/*.md`, `.agents/skills/*/SKILL.md`) are Agent Adapter assets that the Protocol requires to stay out of the project contract (`.protocol/.../AGENTS.md:20-24,28-36`).

### 5.10 memory — N/A

- **Applicability reason:** no agent memory store, namespace, retrieval or retention. In-repo state material is application state (`.agents/rules/client-state-management.md` — TanStack Query/jotai/zustand) and product persistence (`docs/architecture.md:537-612`, `crates/pp-db/src/entities/`).

### 5.11 mcp — N/A

- **Applicability reason:** no MCP configuration or integration; `grep -rni mcp` over `opencode.jsonc` and `.opencode/` returns no match, and no `docs/mcp/` exists. `apps/panel/AGENTS.md:4` merely lists HeroUI's own `mcp-server.mdx` docs entry.

### 5.12 adrs — Partial

- **Evidence:** `docs/adr/0001-0007` with Status/Date/Deciders/Scope headers; ADR-0007 Accepted and explicitly Supersedes ADR-0003, which is marked Superseded by ADR-0007; references from `AGENTS.md:28,278-293` and `README.md:70`.
- **Gaps:** no documented ADR convention/template/index; `docs/index.md` does not list the ADR directory; ADR-0002 is still `Proposed` although its own §8.2 records the v1 implementation as complete; ADR-0006 is dated 2026-12-19, after ADR-0007 (2026-10-03) and after the audited repository state; superseded-path references in ADR-0004/0005/0006 are unannotated historical records.
- **Action (Pass 2):** add the ADR convention/index (ADC-001) and repair metadata (ADC-006).

### 5.13 agent-adapter — Partial

- **Evidence:** `opencode.jsonc:1-66`, `.opencode/AGENTS.md`, `.opencode/agents/{orchestrator,executor,reviewer}.md`, `.opencode/rules/coding-standards.md`, `.pi/master.md`, `.pi/agents/{coder,reviewer,writer}.md`, `.pi/subagent-isolation.json`, `.pi/skills/`, `.agents/rules/*.md`, `.agents/skills/`, `skills-lock.json`, `apps/panel/AGENTS.md`.
- **Gaps:** no `docs/adapters/` documentation of supported agents, permissions/hooks, tool mapping or conformance evidence; `.opencode/rules/coding-standards.md:6-14` prescribes verification commands for the removed `apps/desktop/src-tauri`; `.opencode/AGENTS.md:15` is an unfilled placeholder; `.agents/rules/client-state-management.md:1-3` is titled `(apps/desktop)`; overlapping rule sets (`AGENTS.md`, `.agents/rules/`, `.opencode/rules/`, `.pi/*`, `apps/panel/AGENTS.md`) have no stated precedence; no representative-task evidence that adapters can execute the project workflows.
- **Action (Pass 2):** document adapters (ADC-001/002), repair stale adapter paths, declare rule precedence.

---

## 6. Contradiction register

All contradictions are recorded, none silently resolved. Status `surfaced` means the conflict is documented here and in the mapping; the proposed authority is recorded as an open decision where it is consequential.

| ID | Severity | Contradiction | Sources |
|---|---|---|---|
| C-01 | high | Client architecture: `apps/desktop` + `apps/mobile` (pre-merge) vs `apps/client` (ADR-0007 + implementation) | `docs/architecture.md:355-362,410,428,432`; `README.md:235-236`; `Cargo.toml:17`; `.opencode/rules/coding-standards.md:6-14`; `.agents/rules/client-state-management.md:1-3`; `.pi/plans/2026-10-01-*.md:37` **vs** `docs/adr/0007-*.md:6,14`; `apps/`; `README.md:70,262` |
| C-02 | high | README contradicts itself: structure tree shows `desktop/`,`mobile/` while the crate table and architecture prose use `apps/client` | `README.md:235-236` vs `README.md:70,262` |
| C-03 | medium | Integration tests: `AGENTS.md` says "尚未设置，计划添加 tests/ 目录" while an integration test already exists and is undocumented | `AGENTS.md:376`; `docs/development.md:521-583` **vs** `crates/pp-client/tests/real_core_e2e.rs:1-25` |
| C-04 | medium | Frontend stack versions: React 18 + Vite 6 in docs vs React ^19.2.7 + Vite ^8.2.2 in the manifests | `docs/index.md:71`; `docs/architecture.md:126`; `docs/development.md:361-362` **vs** `apps/panel/package.json:8,54`; `apps/client/package.json:58`; `README.md:262` |
| C-05 | low | `AppState` documented with 2 fields vs 7 declared fields | `AGENTS.md:196-210` **vs** `crates/pp-hub/src/state.rs:77-86` |
| C-06 | low | Proto inventory: docs name only `proto/hub_agent.proto` while `proto/singbox_daemon.proto` also exists and is compiled | `AGENTS.md:186,352` **vs** `proto/singbox_daemon.proto`; `crates/pp-proto/build.rs:12-19` |
| C-07 | medium | Release docs require updating `CHANGELOG.md`, which does not exist at the repository root | `docs/contributing.md:261,350,356-376` **vs** absence of `CHANGELOG.md` |
| C-08 | medium | ADR metadata drift: ADR-0002 `Proposed` though implemented; ADR-0006 dated after ADR-0007 and after the audited state | `docs/adr/0002-*.md:3,700`; `docs/adr/0006-*.md:4`; `docs/adr/0007-*.md:4` |
| C-09 | low | Frontend dependency install convention: root-only `bun install` (single lockfile) vs per-app `cd apps/panel && bun install` instructions | `AGENTS.md:37-39,99-124`; `package.json:4-9` **vs** `README.md:143,290`; `docs/index.md:57` |
| C-10 | medium | Agent instruction precedence/quality: unfilled `.opencode/AGENTS.md` placeholder and multiple overlapping rule sources with no precedence, one of which is stale | `.opencode/AGENTS.md:15`; `.opencode/rules/coding-standards.md`; `.agents/rules/*.md`; `.pi/*`; `apps/panel/AGENTS.md` |

Additional non-contradiction finding: the protocol entry point (`AGENTS.md:7-19`, root `ADOPT.md`) exists only in the dirty working tree, not in the audited revision (see ADC-004).

---

## 7. Open decisions

Recorded in `docs/adoption/protocol-mapping.json` → `open_decisions`.

| ID | Question (abridged) | Recommendation | Blocks |
|---|---|---|---|
| ADC-001 | Keep the existing documentation convention (docs/index.md, docs/architecture.md, docs/adr/, …) or migrate to Protocol-recommended paths (`docs/README.md`, `docs/architecture/`, `docs/workflows/`, `docs/testing/`, `docs/decisions/`)? | Keep existing convention (`allow_existing_convention=true`); add only genuinely absent documents | workflows, adrs, agent-adapter |
| ADC-002 | Is the coding-agent infrastructure in scope for agents/tools/prompts/memory/evaluation, or strictly Agent Adapter scope? | Strictly Agent Adapter scope; document under `docs/adapters/` | evaluation, agents, tools, prompts, memory, mcp, agent-adapter |
| ADC-003 | Source of truth for client architecture: stale docs (`apps/desktop`/`apps/mobile`) or ADR-0007 + implementation (`apps/client`)? | ADR-0007 + `apps/` is authoritative; correct docs in Pass 2 | architecture, project-contract |
| ADC-004 | Commit the uncommitted protocol entry point before Pass 2, or keep it working-tree only? | Commit as one docs/chore change after audit acceptance | project-contract |
| ADC-005 | Must the Definition of Done require automated frontend tests (currently none)? | Document the current gate; decide testing separately | testing, workflows |
| ADC-006 | ADR metadata convention and repair of ADR-0002 status / ADR-0006 date? | Add `docs/adr/README.md` with status vocabulary, index and template; repair metadata | adrs |
| ADC-007 | Create the missing root `CHANGELOG.md` or correct the release instructions? | Decide explicitly; either add a changelog starting at 0.4.5 or drop the step | project-contract, operations |

---

## 8. Proposed Pass 2 changes (not executed)

Dry-run of the generation step; nothing below was written during Pass 1.

- **Create:** `docs/adoption/adoption-report.json` + `adoption-report.md` (Pass 2 outcome), workflow documentation in the location chosen by ADC-001, `docs/adr/README.md` (index/convention), `docs/adapters/` documentation, optional `docs/README.md` if ADC-001 chooses the Protocol path.
- **Update:** `README.md` (structure tree), `docs/index.md` (index wiring, client entry, stack versions), `docs/architecture.md` (client chapters, stack versions, AppState excerpt), `docs/development.md` (test surface, stack versions), `AGENTS.md` (Definition of Done, escalation, integration-test statement, proto inventory), `.opencode/rules/coding-standards.md` and `.agents/rules/client-state-management.md` (stale paths), `docs/adr/0002`/`0006` metadata per ADC-006.
- **Preserve intentionally:** `docs/architecture.md` file location, `docs/adr/` convention, `docs/index.md`, all ADR historical records (annotate, do not rewrite history), `.agents/skills/` and `.pi/skills/` vendor content, `docs/plans/` and `docs/research/` historical documents.
- **Explicitly omitted (N/A):** `docs/evaluation/`, `docs/agents/`, `docs/tools/`, `docs/prompts/`, `docs/memory/`, `docs/mcp/` — creating these would produce empty placeholders, which `mapping.md:61` forbids.

---

## 9. Validation results

### 9.1 Structural / schema validation (run, passed)

Validated with `ajv` 2020-12 against the Protocol schemas in `.protocol/agent-development-protocol/schemas/`:

```
PASS  docs/adoption/project-inventory.json  <-  project-inventory.schema.json
PASS  docs/adoption/protocol-mapping.json   <-  protocol-mapping.schema.json
   status-enum=true unique-ids=true evidence-on-nonNA=true reason-on-NA=true capabilities=13
   summary={"Partial":6,"N/A":6,"Existing":1} open_decisions=7
```

- `adoption-report.schema.json` was **not** applied: Pass 1 does not produce an adoption report (`validation.md:77-89`); it is a Pass 2 artifact.
- Additional structural checks executed: exactly one status per manifest capability, all 13 manifest capability ids present, status vocabulary valid, evidence present on every non-N/A classification, explicit reason on every N/A classification.

### 9.2 Evidence reference validation (run, passed)

Every path-like token in the `evidence`/`target_documents` fields of both artifacts was resolved against the repository: **92 distinct repository paths verified to exist**; 4 remaining tokens are glob patterns whose expansions were manually confirmed (`*.md`, `{coder,reviewer,writer}.md`, `{orchestrator,executor,reviewer}.md`, `.agents/skills/*/SKILL.md`). Directories marked "(proposed)" are intentionally absent. No stale evidence reference remains.

### 9.3 Project validation

Intentionally **not** run, per `validation.md:52`: Pass 1 changed no project file, so build/test/lint validation would not validate anything. This is recorded rather than substituted by schema validation. Project validation belongs to Pass 2 (and to the project's own CI: `scripts/check-rust-gates.sh`, `.github/workflows/ci.yml`).

### 9.4 Pass 1 acceptance checklist (`acceptance-criteria.md:5-21`)

| Criterion | Result |
|---|---|
| Repository revision and protocol version recorded | ✅ `1bf847a…`, protocol 0.1.0, submodule `310fba5…` |
| Discovery covers relevant repository areas | ✅ section 3 |
| Every manifest capability has exactly one status | ✅ 13/13 (validated) |
| Status based on substantive capability, not path presence | ✅ `mapping.md:32` applied; no Missing classification |
| Every Existing/Partial/Missing has evidence | ✅ 7/7 non-N/A items (validated) |
| Every N/A has a reason | ✅ 6/6 (validated) |
| Contradictions recorded | ✅ C-01…C-10 |
| Consequential decisions recorded as open decisions | ✅ ADC-001…ADC-007 |
| No source code, README, AGENTS.md or project-contract file modified | ✅ section 10 |
| Inventory and mapping are valid JSON | ✅ |
| Inventory and mapping conform to their schemas | ✅ |
| Evidence paths/references spot-checkable | ✅ 9.2 |

---

## 10. Pass 1 write-boundary confirmation

Pass 1 wrote **only** the three audit artifacts:

- `docs/adoption/project-inventory.json`
- `docs/adoption/protocol-mapping.json`
- `docs/adoption/audit-pass1.md`

Confirmed not modified: any source code; `README.md`; `AGENTS.md`; existing ProxyPanel documentation under `docs/` (including ADRs); the Protocol submodule `.protocol/agent-development-protocol/`; any configuration, CI, script or build file. No Protocol file was copied into the ProxyPanel tree. `git status` after Pass 1 shows the pre-existing `AGENTS.md` modification and untracked `ADOPT.md` unchanged, plus the three new untracked audit files under `docs/adoption/`.

Pass 2 was **not** started: no documentation generation, reconciliation or adoption-report artifacts were produced.

---

## 11. Known limitations

- The audit was performed against a dirty working tree; the protocol section in `AGENTS.md:7-19` and root `ADOPT.md` are not part of revision `1bf847a…`. Evidence for those items is working-tree evidence (flagged per item and in ADC-004).
- The `docs/plans/` and `docs/research/` documents are dated working records (2026-08 … 2026-10); they were treated as historical evidence, not current contract.
- No project build/test was executed (read-only audit), so claims about test/CI behaviour rest on configuration evidence rather than observed runs.
- Classification of the coding-agent infrastructure as Agent Adapter scope (ADC-002) is a judgement call by the auditor; the alternative reading would change `evaluation` to `Missing`/`Partial` and add capabilities under agents/tools/prompts/memory.
- The Protocol itself supplies no project-specific taxonomy for non-Agent projects, so `N/A` reasons for the Agent-family capabilities were written explicitly to keep the boundary reviewable.

---

## 12. Pass 1 outcome

Pass 1 is **complete and non-conformant by design** (audit only): discovery evidence recorded, all 13 capabilities classified with evidence or explicit N/A reasons, 10 contradictions surfaced, 7 consequential decisions recorded as open decisions, machine-readable artifacts schema-valid, and no project file or submodule modified. Pass 2 must not start until the open decisions — at minimum ADC-001, ADC-002 and ADC-003 — are resolved.
