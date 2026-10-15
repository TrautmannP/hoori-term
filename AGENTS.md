# hoori-term agent working agreement

## Operating model

This agreement covers three separately started agent sessions: development,
concept, and project management. Operate continuously on the product, with
scope and delivery order maintained in Linear.

Project management may autonomously maintain issues, priorities, dependencies,
projects, and milestones within the clarified concept. Patrick owns unresolved
product decisions. HOO-5 is entirely an open draft to clarify together; none of
its product or technology proposals is implicitly approved.

Development may commit, push, create pull requests, and merge after successful
local technical checks. GitHub pipeline setup is explicitly deferred. Patrick
performs product review using runnable Ubuntu builds. Windows runtime validation
remains pending until Patrick supplies the VM.

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
  Its initial contents are entirely an open concept draft. Record decisions
  explicitly as they are clarified with Patrick.
- At setup, HOO-5 is the only issue, has no project, and is in `Needs Concept`.
  No Linear project exists in this team yet. Revalidate this snapshot before
  creating a project or assigning issues; never create a duplicate by name alone.
- The repository root is `/home/patrick/Projekte/hoori-term`. Recover its current
  state from live Git status, branches, history, and remote configuration.
- GitHub repository: [TrautmannP/hoori-term](https://github.com/TrautmannP/hoori-term),
  with `main` as the integration branch. Verify the remote before delivery.
- Send new product feedback to the project-management session or the relevant
  Linear issue. Answer concept questions in the concept session or the issue;
  that session records the resulting decision for the other roles.

## Three roles

Each operating session has exactly one role, selected by its user-issued goal.
Do not take another role merely because its queue is empty.

### Development

- Implement eligible implementation issues in `Ready for dev`, following the
  order selected by project management and their verified dependencies. Read the
  project's working queues; take the first eligible development issue. If no
  explicit order exists, use priority, then oldest creation time among eligible
  issues. Respect any explicit user-selected issue.
- Work on one implementation issue at a time. Claim it as `In Progress` and
  record role, session/run identifier, branch, and planned scope in an issue
  comment. Re-read to confirm ownership before editing source.
- Read the approved concept and acceptance criteria. Resolve routine technical
  choices within that scope; route material product questions to concept work.
  If a blocking concept gap appears during implementation, preserve the branch
  and evidence, document the question, and hand the issue back to `Needs Concept`.
- This role owns source changes, implementation status, and delivery evidence.
  Concept and project-management sessions may inspect source, but must not edit
  it or change the developer's branch, index, or worktree.
- Follow the Git delivery and review-build policies below. Report the exact
  branch, commit, changed behavior, checks, and remaining limits.
- Move an issue to `Human Review` only with a concrete reviewable result and
  technical acceptance evidence after merge. Patrick's product acceptance is
  required before `Done`; project management records that acceptance in Linear.
- HOO-5 is an umbrella concept. Do not treat it as a single implementation task,
  even if its concept is later moved to `Ready for dev`.

### Concept

- Start with HOO-5, then process `Needs Concept` issues in the planning order.
  Clarify product goals, the first useful increment, target platforms, and
  technical constraints before treating the proposed roadmap as settled.
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
- Work incrementally: clarify enough to specify the first useful implementation
  slice, then refine later slices. Unrelated future decisions need not prevent
  a bounded, explicitly approved prototype.

### Project management and feedback

- Autonomously maintain issues, delivery order, priorities, dependencies,
  milestones, projects, and feedback intake within the clarified concept.
  Route unresolved product decisions to Patrick through concept work.
- Use HOO-5 as the product anchor. Find or create the `hoori-term` project in the
  HOO team after setup and attach HOO-5; check existing records first. Derive
  milestones and implementation commitments from clarified decisions. Label
  tentative plans as proposals.
- Maintain a short working-queues section in the project description containing
  the next concept issue and up to three ordered development issue IDs. Use live
  statuses and dependencies to keep it current; identify waiting prerequisites
  when no development issue is eligible. Preserve the rest of the description.
- Prefer small, independently verifiable issues that produce useful increments.
  Plan the current milestone in detail and keep later work coarser.
- Choose the next development issue using user priorities, prerequisites,
  technical risk, and milestone outcomes. Verify the evidence for each dependency
  using the dependency rules below.
- Own issue creation and planning metadata. Coordinate material changes to
  requirements with the concept role and changes to active scope with the
  developer; avoid modifying an active implementation's acceptance silently.
- Capture feedback that Patrick sends to this session or records in Linear.
  Check for duplicates, preserve the actual request and its context, and link
  affected issues. Do not claim access to messages in other independent sessions.
  Record the reported build's full commit hash, platform, reproduction steps,
  expected and actual behavior, and links to the original delivery when known.
  Ask for missing information only when needed; do not discard incomplete feedback.
- Route unclear requests to `Needs Concept`; keep unprioritized future work in
  `Backlog`. The concept role owns promotion to `Ready for dev`.
- Do not invent user feedback, implementation progress, approvals, delivery
  dates, or additional product commitments to keep the loop busy.
- Record `Done` only after Patrick's explicit acceptance. If feedback reveals
  an unmet acceptance criterion, track the correction and retain the unresolved
  original acceptance. Report merged work and accepted work separately.

## Status and handoff rules

Use the team's actual status names. Revalidate them at the beginning of a run.

| Status | Working meaning | Transition owner |
| --- | --- | --- |
| `Backlog` | Captured, not selected for clarification or development | Project management |
| `Needs Concept` | Requirements or decisions need concept work | Concept; project management routes work here |
| `Ready for dev` | Concept complete and implementation scope reviewable | Concept |
| `In Progress` | Claimed implementation work | Development |
| `Human Review` | Implementation delivered and awaiting agreed review | Development hands off |
| `Done` | Explicitly accepted by Patrick | Patrick; project management records acceptance |
| `Canceled` / `Duplicate` | Explicitly closed with explanation or duplicate link | Within agreed planning authority |

- `Todo` currently also exists. Do not use it as an additional implicit queue.
- `Human Review` currently has Linear's `completed` status type. Check the exact
  status name and actual integration/review evidence; `completedAt` or
  `statusType` alone must not satisfy acceptance or milestone gates. Re-check
  this configuration; it may be changed to a `started` status type later.
- A technical dependency is usable when its required changes are merged into
  the selected base and its relevant technical checks pass. Pending product
  review does not by itself block such follow-up work. An explicit user-approval
  dependency or milestone acceptance gate requires Patrick's acceptance.
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

## Product discovery and engineering

- HOO-5 proposes a local-first native Rust terminal for Ubuntu and Windows,
  with no required account, cloud dependency, or telemetry. The proposed stack
  is iced/wgpu, alacritty_terminal, portable-pty, and TOML configuration.
  These are discussion inputs, not settled implementation requirements.
- Validate the selected GUI/rendering integration early. Record evidence before
  treating a proposed library choice as proven. Keep logical boundaries clear
  without creating every proposed crate before it is useful.
- Keep UI, terminal state, PTY/session lifecycle, commands, and persistence
  separate enough to test. PTY I/O must not block the UI; inactive sessions
  still consume their output. Bound queues and scrollback.
- The draft distinguishes tab identity from position/title, pinning from compact
  presentation, and restored configuration from persistent processes. Preserve
  those distinctions in concept discussions and record the resulting decisions.
- Treat terminal output and escape sequences as untrusted data. Preserve
  explicit user control of clipboard access and configured automatic commands.
  Do not log terminal contents, secrets, or sensitive input by default.
- Respect the MVP boundary agreed through concept work. The draft places
  workspaces, splits, and a session daemon after its MVP and excludes cloud
  features and an integrated AI assistant. Their scope is not yet approved;
  do not silently add them to the first implementation increment.

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
- Keep the per-issue gate fast: cover new behavior with Rust tests at the
  state/model level (and real-PTY tests where bytes matter). Do not add a new
  per-feature GUI automation script, and do not drive the UI by OCR. Real-window
  X11 runs are limited to `scripts/gui-smoke.py` when window/rendering/input
  paths change and `scripts/gui-ubuntu-qualification.py` for milestone
  qualification. Visual and feature acceptance happens on Patrick's review build.
  Patrick decided this on 2026-10-16 after per-issue GUI runs took up to an hour.
- Distinguish compile checks, automated runtime tests, GUI checks, and manual
  platform evidence. Linux or cross-compilation does not prove Windows runtime
  behavior. Missing Windows/WSL or display access remains an explicit gap.
- Record enough evidence to reproduce acceptance. Never report an unrun check
  or unavailable environment as passing.

## Git delivery and product acceptance

- Use an issue-specific branch, normally `codex/hoo-<number>-<short-description>`.
  For initial repository setup, use a dedicated setup branch. Keep commits scoped
  to the owned issue and preserve unrelated changes.
- Development is authorized to commit, push, open a pull request, and merge it
  into `main` once relevant local checks and required CI checks pass. Use the
  repository's normal merge policy and respect branch protection.
- Do not set up a GitHub Actions workflow or another GitHub CI pipeline.
  Patrick has explicitly deferred this work. Use documented local checks;
  pipeline setup is out of scope until Patrick explicitly requests it.
- Explain the resulting behavior and validation in the PR. For documentation-only
  work, review the content and run `git diff --check`; an application test suite
  is not required before an application exists.
- After merge, verify the resulting main commit and publish the corresponding
  review build where the issue changes runnable behavior. Link the PR, final
  commit, checks, and launch artifact in Linear and the user-facing handoff.
- Technical merge and product acceptance are separate events. Move delivered
  work to `Human Review`; record `Done` after Patrick explicitly accepts it.
  Other eligible work can continue while product review is pending.

## Test environments and runnable review builds

- Use the Ubuntu system available in this workspace. Patrick can also test it
  manually. Record the actual display backend and environment used; testing one
  Ubuntu session does not prove both Wayland and X11 behavior.
- Patrick is preparing a Windows VM. Until access is explicitly supplied,
  Windows, PowerShell, ConPTY, and WSL runtime checks are unverified. Continue
  eligible Ubuntu work and track pending Windows evidence in linked issues and
  milestone criteria. Do not silently drop agreed Windows acceptance criteria.
- Establish a runnable Ubuntu prototype early. Before one exists, state plainly
  which deliveries are scaffolding or documentation and track the first runnable
  increment. Do not present a placeholder as a working application.
- Once the application is runnable, each delivery that changes runnable behavior
  must provide a versioned Ubuntu build and a convenient local launcher. Verify
  that the launcher opens the intended version, or record the exact limitation
  if a GUI check needs Patrick's active desktop session.
- Store review artifacts outside disposable worktrees and generated source
  paths. A suitable convention is
  `/home/patrick/Projekte/hoori-term-builds/<full-commit-hash>/`. Keep previous
  builds available for feedback and comparison; do not overwrite a versioned build.
- Link the runnable build/launcher using an absolute local file link and provide
  the exact single command to start it. Provide a desktop launcher when useful.
  Include the full source commit, platform, prerequisites, and checks performed.
- Build review versions from clean committed source at the final merged commit.
  Embed the commit identity at build time so it remains correct after a branch
  switch or launch from another directory. Show at least the short commit hash
  persistently in the application window title, e.g. `hoori-term [abc1234]`.
  Dynamic shell titles must retain this build identifier. Make the full hash
  available with the build metadata. Distinguish any local dirty build explicitly.
- Project management must preserve this commit identity when turning feedback
  into issues so the developer can reproduce the reported version.

## Continuous operation

- Each goal belongs to its own session. Use Linear for durable coordination;
  there is no assumed shared conversation or automatic wake-up of another goal.
- Re-read relevant queues and handoffs between work items. During idle periods,
  recheck no more often than once per minute and use bounded, interruptible waits
  supported by the running environment. Split waits into at most 60 seconds per
  call so user input can be handled promptly. Avoid tight polling, repeated
  no-change writes, and invented housekeeping.
- An empty personal queue is not evidence that the whole product is complete.
  Preserve a concise checkpoint with pending items, next dependency, and where
  work should resume. Follow the runtime's rules for repeated blockers.
- A goal remains subject to its runtime, budget, user stop/pause, and external
  blockers. Do not promise unattended monitoring after that session stops.
- Resume from live issues, recorded decisions, and actual Git state. Do not
  repeat completed work or overwrite another agent's current assignment.
