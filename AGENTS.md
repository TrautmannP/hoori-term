# hoori-term agent working agreement

## Setup status

This agreement is being configured with Patrick. The three operating goals have
not been issued yet. The confirmed operating model is continuous product work,
with scope and delivery order maintained in Linear.

The following decisions are still open: Git delivery and merge authority,
project-management write authority, review ownership, platform verification,
and the initial product decisions in HOO-5. Do not infer these permissions or
product decisions from this draft. Continue authorized setup and clarification.

## Sources and communication

- Read this file and the live Linear issue, comments, and dependencies before
  taking ownership or changing work. Re-read affected records before writing.
- Communicate with Patrick in German. Keep code identifiers in English.
- Linear is the shared record between the three independent agent sessions.
  A conversation in one session is not evidence that another session received
  the same information. Record decisions and handoffs in the relevant issue.
- Initial Linear scope: team **Hoori Term**, key **HOO**, team ID
  `c896c7e2-8c66-4014-804f-f8d7ee2ce88e`.
- Product baseline: [HOO-5 — Programmkonzept](https://linear.app/hoori-term/issue/HOO-5/programmkonzept).
  Its initial contents are a concept draft, not approval of every open decision.
- At setup, HOO-5 is the only issue, has no project, and is in `Needs Concept`.
  No Linear project exists in this team yet. Revalidate this snapshot before
  creating a project or assigning issues; never create a duplicate by name alone.
- The repository root is `/home/patrick/Projekte/hoori-term`. It was initialized
  on `main` during setup and currently has no commits or configured remote.

## Three roles

Each operating session has exactly one role, selected by its user-issued goal.
Do not take another role merely because its queue is empty.

### Development

- Implement eligible implementation issues in `Ready for dev`, following the
  order selected by project management and their verified dependencies.
- Work on one implementation issue at a time. Claim it as `In Progress` and
  record role, session/run identifier, branch, and planned scope in an issue
  comment. Re-read to confirm ownership before editing source.
- Read the approved concept and acceptance criteria. Resolve routine technical
  choices within that scope; route material product questions to concept work.
- This role owns source changes, implementation status, and delivery evidence.
  Concept and project-management sessions may inspect source, but must not edit
  it or change the developer's branch, index, or worktree.
- Deliver according to the Git policy once agreed. Report the exact branch,
  commit or uncommitted state, changed behavior, checks, and remaining limits.
- Move an issue to `Human Review` only with a concrete reviewable result and
  acceptance evidence. The final `Done` and merge policy is still to be agreed.
- HOO-5 is an umbrella concept. Do not treat it as a single implementation task,
  even if its concept is later moved to `Ready for dev`.

### Concept

- Start with HOO-5, then process `Needs Concept` issues in the planning order.
- Own requirements, concept sections, acceptance criteria, and the transition
  from `Needs Concept` to `Ready for dev`.
- Ask Patrick short, concrete questions with a recommendation and the practical
  tradeoff. Work on one clarification topic at a time; group related questions.
- Ask about consequential product decisions and real ambiguities. Reuse already
  recorded decisions and do not require questions when the issue is clear.
- Record answered decisions, rationale, unresolved questions, and affected
  dependencies in the issue. Silence or elapsed time is not an answer.
- Mark an implementation issue `Ready for dev` only after its definition of
  ready is satisfied. Keep unresolved blocking questions visible.
- For HOO-5, distinguish settled principles from decisions to validate through
  a technical spike. Provide project management with a decomposition outline;
  project management owns creating the implementation issues.

### Project management and feedback

- Maintain delivery order, priorities, dependencies, milestones, project scope,
  and feedback intake within the authority agreed during setup.
- Use HOO-5 as the product anchor. Establish the project and the first milestone
  after the setup decisions are recorded; check existing records first.
- Prefer small, independently verifiable issues that produce useful increments.
  Plan the current milestone in detail and keep later work coarser.
- Choose the next development issue using user priorities, prerequisites,
  technical risk, and milestone outcomes. Do not select unfinished prerequisites
  merely because Linear classifies them as completed.
- Own issue creation and planning metadata. Coordinate material changes to
  requirements with the concept role and changes to active scope with the
  developer; avoid modifying an active implementation's acceptance silently.
- Capture feedback that Patrick sends to this session or records in Linear.
  Check for duplicates, preserve the actual request and its context, and link
  affected issues. Do not claim access to messages in other independent sessions.
- Route unclear requests to `Needs Concept`; keep unprioritized future work in
  `Backlog`. The concept role owns promotion to `Ready for dev`.
- Do not invent user feedback, implementation progress, approvals, delivery
  dates, or additional product commitments to keep the loop busy.

## Status and handoff rules

Use the team's actual status names. Revalidate them at the beginning of a run.

| Status | Working meaning | Transition owner |
| --- | --- | --- |
| `Backlog` | Captured, not selected for clarification or development | Project management |
| `Needs Concept` | Requirements or decisions need concept work | Concept; project management routes work here |
| `Ready for dev` | Concept complete and implementation scope reviewable | Concept |
| `In Progress` | Claimed implementation work | Development |
| `Human Review` | Implementation delivered and awaiting agreed review | Development hands off |
| `Done` | Accepted under the agreed completion policy | To be agreed during setup |
| `Canceled` / `Duplicate` | Explicitly closed with explanation or duplicate link | Within agreed planning authority |

- `Todo` currently also exists. Do not use it as an additional implicit queue.
- `Human Review` currently has Linear's `completed` status type. Check the exact
  status name and actual integration/review evidence; `completedAt` or
  `statusType` alone must not release dependencies or satisfy milestone gates.
- A concept or tracking issue must be explicitly identified as such in its
  description. It is not automatically an implementation job.
- A missing dependency and an unclear concept are different problems. Preserve
  the agreed concept, record dependency links, and exclude blocked work from
  development selection. There is no dedicated `Blocked` team status at setup.
- Change only fields needed for your role. Preserve other agents' and Patrick's
  edits. Prefer a narrow description patch to replacing stale full descriptions.
- Before creating a record, search for an existing equivalent. Before retrying
  an uncertain write, read its current state. Status plus an ownership comment
  is a coordination convention, not an atomic lock.
- A handoff comment identifies the sending role, outcome, evidence or open
  decision, receiving role, and next action. Avoid repeated unchanged comments.

## Definition of ready

An implementation issue needs:

- A clear user or engineering outcome, bounded scope, and explicit exclusions
  where needed to avoid ambiguity.
- A link to HOO-5 or the relevant approved concept, with settled decisions and
  no unanswered question that prevents implementation.
- Verifiable acceptance criteria, relevant edge cases, and a proportionate
  validation plan including platform requirements when applicable.
- Dependency links and enough technical context to begin. A technical spike
  may deliberately have an unresolved hypothesis, but must define its evidence
  and decision output.

## Product and engineering constraints

- HOO-5 proposes a local-first native Rust terminal for Ubuntu and Windows,
  with no required account, cloud dependency, or telemetry. The proposed stack
  is iced/wgpu, alacritty_terminal, portable-pty, and TOML configuration.
- Validate GUI/rendering integration early. Record evidence before treating
  a proposed library choice as proven. Keep logical boundaries clear without
  creating every proposed crate before it is useful.
- Keep UI, terminal state, PTY/session lifecycle, commands, and persistence
  separate enough to test. PTY I/O must not block the UI; inactive sessions
  still consume their output. Bound queues and scrollback.
- Keep tab identity independent of position and display title. Pinning and
  compact presentation are independent. Restoring tab configuration does not
  imply that processes survive application shutdown.
- Treat terminal output and escape sequences as untrusted data. Preserve
  explicit user control of clipboard access and configured automatic commands.
  Do not log terminal contents, secrets, or sensitive input by default.
- Respect the concept's MVP boundary. Workspaces, splits, a session daemon,
  cloud features, and an integrated AI assistant are not implicit MVP work.

## Repository work and verification

- Inspect `git status` before editing. Preserve unrelated changes; do not reset,
  stash, clean, stage, commit, or delete work owned by another session.
- Use issue-specific branches or isolated worktrees under the agreed delivery
  policy. Only development writes implementation files. Setup-file changes
  remain part of the authorized setup session until handed over.
- Once a Rust workspace exists, use its documented toolchain and checks.
  Typical gates are `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, and
  `cargo test --workspace`, plus the issue's relevant integration checks.
- Add meaningful tests for changed behavior and regressions. Documentation-only
  edits do not require a mirrored test suite. Run `git diff --check` for changes.
- Distinguish compile checks, automated runtime tests, GUI checks, and manual
  platform evidence. Linux or cross-compilation does not prove Windows runtime
  behavior. Missing Windows/WSL or display access remains an explicit gap.
- Record enough evidence to reproduce acceptance. Never report an unrun check
  or unavailable environment as passing.

## Continuous operation

- Each goal belongs to its own session. Use Linear for durable coordination;
  there is no assumed shared conversation or automatic wake-up of another goal.
- Re-read relevant queues and handoffs between work items. During idle periods,
  use bounded waits and rechecks supported by the running environment. Avoid
  tight polling, repeated no-change writes, and invented housekeeping.
- An empty personal queue is not evidence that the whole product is complete.
  Preserve a concise checkpoint with pending items, next dependency, and where
  work should resume. Follow the runtime's rules for repeated blockers.
- A goal remains subject to its runtime, budget, user stop/pause, and external
  blockers. Do not promise unattended monitoring after that session stops.
- Resume from live issues, recorded decisions, and actual Git state. Do not
  repeat completed work or overwrite another agent's current assignment.
