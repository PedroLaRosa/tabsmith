export const meta = {
  name: 'hexagonal-phase',
  description: 'Implement one phase of the tabsmith hexagonal refactor step by step, then adversarially review, verify and fix',
  whenToUse: 'Run once per phase of docs/hexagonal-refactor-plan.md with args {phase, startRev, steps, reviewNotes}',
  phases: [
    { title: 'Implement', detail: 'one agent per plan step, sequential, gate + commit each' },
    { title: 'Review', detail: 'behaviour, test-lock, architecture, completeness lenses' },
    { title: 'Verify', detail: 'one skeptic per finding' },
    { title: 'Fix', detail: 'apply confirmed findings, gate, commit' },
  ],
}

// Run from the repo root: Workflow({scriptPath: 'docs/hexagonal-refactor/phase-workflow.js', args: {phase, startRev, steps,
// reviewNotes}}). See docs/hexagonal-refactor/README.md. Tool and research paths are relative to the repo root.
const S = 'scripts/hexagonal-refactor'
const BRIEF = `First read docs/hexagonal-refactor/agent-brief.md and follow it exactly (it is the contract for this refactor). Then read the parts of docs/hexagonal-refactor-plan.md and docs/ubiquitous-language.md relevant to your task.`
const { phase: PHASE, startRev, steps, reviewNotes } = args

const STEP_SCHEMA = {
  type: 'object',
  properties: {
    gatePassed: { type: 'boolean', description: 'true only if the last full gate.sh run printed GATE PASS and everything is committed' },
    commits: { type: 'array', items: { type: 'string' }, description: 'hash + subject of each commit you made' },
    summary: { type: 'string', description: 'what you did, which files were created/moved/deleted' },
    changedTests: { type: 'array', items: { type: 'object', properties: { name: { type: 'string' }, justification: { type: 'string' } }, required: ['name', 'justification'] } },
    deviations: { type: 'array', items: { type: 'string' }, description: 'every deviation from the plan and why' },
    concerns: { type: 'array', items: { type: 'string' }, description: 'suspicious things noticed but not fixed, risks for later steps' },
  },
  required: ['gatePassed', 'commits', 'summary', 'changedTests', 'deviations', 'concerns'],
}

const FINDINGS_SCHEMA = {
  type: 'object',
  properties: {
    findings: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          title: { type: 'string' },
          severity: { type: 'string', enum: ['behaviour-change', 'test-lock', 'build-break', 'plan-deviation', 'architecture', 'nit'] },
          file: { type: 'string' },
          line: { type: 'integer' },
          detail: { type: 'string', description: 'what is wrong and why it matters' },
          evidence: { type: 'string', description: 'exact code/diff/command output proving it' },
          suggestedFix: { type: 'string' },
        },
        required: ['title', 'severity', 'file', 'detail', 'evidence', 'suggestedFix'],
      },
    },
  },
  required: ['findings'],
}

const VERDICT_SCHEMA = {
  type: 'object',
  properties: {
    refuted: { type: 'boolean' },
    mustFix: { type: 'boolean', description: 'true if real and it must be fixed before the phase can end (behaviour change, test-lock violation, build break, clear plan/architecture violation). false for nits or debatable style.' },
    reasoning: { type: 'string' },
  },
  required: ['refuted', 'mustFix', 'reasoning'],
}

const brief = r => r ? `- ${r.summary}\n  commits: ${r.commits.join('; ')}\n  deviations: ${r.deviations.join(' | ') || 'none'}\n  concerns: ${r.concerns.join(' | ') || 'none'}` : '- (no report)'

phase('Implement')
const stepResults = []
for (const step of steps) {
  const prior = stepResults.map((r, i) => `Step ${steps[i].id}:\n${brief(r)}`).join('\n')
  const r = await agent(
    `${BRIEF}\n\nYou implement ${PHASE}, step ${step.id}: ${step.title}.\n\n${step.prompt}\n\n` +
    (prior ? `Earlier steps of this phase reported:\n${prior}\n\n` : '') +
    `Work in the repo's working tree on branch refactor/hexagonal-architecture (check \`git status\` first; it should be clean). ` +
    `Finish with everything committed and a full \`${S}/gate.sh\` run printing GATE PASS. If you cannot get the gate green without changing behaviour, stop, leave the tree committed at the last green state (git stash/checkout your broken work), set gatePassed=false and explain.`,
    { label: `${PHASE} step ${step.id}`, phase: 'Implement', schema: STEP_SCHEMA },
  )
  stepResults.push(r)
  if (!r || !r.gatePassed) {
    log(`Step ${step.id} did not finish green; stopping the phase for the orchestrator.`)
    return { stoppedAt: step.id, stepResults }
  }
}

const LENSES = [
  {
    key: 'behaviour',
    prompt: `Lens: BEHAVIOUR PRESERVATION of production code. For every production (non-test) change in \`git diff ${startRev}..HEAD\`, find the original code (\`git show ${startRev}:<old path>\`) and compare. Moved code must be identical apart from paths, visibility and glossary renames. Flag any change in logic, control flow, error messages/contexts, stdout/stderr text, exit codes, env var handling, file paths, file modes, write ordering, symlink handling, timing/timeouts, thread behaviour, or iteration order. Pay special attention to anything the plan calls "dedupe" or "unify": prove both former callers still get exactly the old behaviour. Check #[cfg(windows)]/#[cfg(target_os = "linux")] code by hand: would it still compile on those targets (every path resolves from its new location)?`,
  },
  {
    key: 'test-lock',
    prompt: `Lens: TESTS ARE LOCKED. Run \`python3 ${S}/testlock.py . ef807c1 --diff\` and judge every CHANGED test: it is allowed only if it is pure restructuring (paths, glossary renames, moved file, split into parts that together assert exactly the same, shared temp-dir helper, same scenario built through a new constructor). Any change to what is asserted, to inputs, or to the scenario is a violation. testlock only covers #[test] fn bodies: also diff the non-test helper fns inside test modules (fixtures, builders, fake bins) against ${startRev} and flag behaviour changes there. Check that no test was deleted, renamed, #[ignore]d or weakened, and that every new test follows testing-standard (should_… names, AAA, no mocks) and actually exercises the new seam.`,
  },
  {
    key: 'architecture',
    prompt: `Lens: ARCHITECTURE CONFORMANCE. Check the code this phase created against docs/hexagonal-refactor-plan.md sections 1, 2 and 4 and docs/ubiquitous-language.md: file locations and names (one type per snake_case file, small private helpers may stay with their owner), mod.rs files only declare children (no glob re-exports), visibility at most pub(crate), domain struct fields private, domain uses std (+unicode_width) only, application uses no infrastructure/external crates, no cross-module use-case/infrastructure imports, shared imports no feature module, anyhow only in infrastructure/cli, infrastructure/pty and bootstrap, no println!/eprintln!/logging in domain or application, ports/adapters/use cases named per the plan's naming rules, glossary names used. Also review src/architecture_tests.rs: does it really enforce the plan's four rules for every registered module (think of a violation it would miss)? Apply the coding-standards skill to new code (naming, small functions, no dead abstractions).`,
  },
  {
    key: 'completeness',
    prompt: `Lens: COMPLETENESS against the plan. List every bullet of ${PHASE} in docs/hexagonal-refactor-plan.md section 5, plus the section 4 layout entries it implies, and check each one is done in the tree (or that the step reports justify a deviation convincingly). Check the phase's "done when" criterion, that shims created in this phase were deleted, that commits follow the convention (refactor(<module>): <why>, trailer present, one commit per step, green/refactor commits for test-first seams), and that the tree is clean. Also hunt for leftovers: duplicated code that should have moved instead of being copied, old helpers still used where the plan says they are replaced.`,
  },
]

const seen = new Set()
const fixes = []
const allConfirmed = []
for (let round = 1; round <= 3; round++) {
  phase('Review')
  const stepNotes = stepResults.map((r, i) => `Step ${steps[i].id} (${steps[i].title}):\n${brief(r)}`).join('\n')
  const fixNotes = fixes.length ? `\nFix rounds so far:\n${fixes.map(brief).join('\n')}\n` : ''
  const verified = await pipeline(
    LENSES,
    lens => agent(
      `${BRIEF}\n\nYou are a REVIEWER of ${PHASE} of the refactor (diff: \`git diff ${startRev}..HEAD\`; also read the step reports below). Do NOT modify the working tree or commit. If you need to experiment, copy the repo (without target/) into a fresh dir under target/hexagonal-refactor/review-tmp/ and work there.\n\n${lens.prompt}\n\n${reviewNotes || ''}\n\nStep reports:\n${stepNotes}\n${fixNotes}\nReport only real, specific problems with evidence. Return an empty list if you find none. Do not report things already reported as fixed unless the fix is wrong.`,
      { label: `review:${lens.key} r${round}`, phase: 'Review', schema: FINDINGS_SCHEMA },
    ),
    (review, lens) => parallel((review?.findings || [])
      .filter(f => { const k = `${f.file}|${f.title}`.toLowerCase(); if (seen.has(k)) return false; seen.add(k); return true })
      .map(f => () => agent(
        `${BRIEF}\n\nYou are a SKEPTIC. A reviewer (${lens.key} lens) of ${PHASE} (diff \`git diff ${startRev}..HEAD\`) claims:\n\n${JSON.stringify(f, null, 2)}\n\nTry hard to refute it by reading the actual code, the original at ${startRev}, and running commands if useful (do not modify the working tree; experiment in a copy under target/hexagonal-refactor/review-tmp/). refuted=true if the claim is wrong, already handled, or not a real problem. If it is real, refuted=false and decide mustFix.`,
        { label: `verify:${lens.key}:${(f.file || '').split('/').pop()}`, phase: 'Verify', schema: VERDICT_SCHEMA, effort: 'high' },
      ).then(v => ({ ...f, lens: lens.key, verdict: v })))),
  )
  const confirmed = verified.flat().filter(Boolean).filter(f => f.verdict && !f.verdict.refuted)
  allConfirmed.push(...confirmed)
  const mustFix = confirmed.filter(f => f.verdict.mustFix)
  log(`Round ${round}: ${confirmed.length} confirmed findings, ${mustFix.length} must-fix.`)
  if (!confirmed.length) break

  phase('Fix')
  const fix = await agent(
    `${BRIEF}\n\nYou FIX review findings for ${PHASE} of the refactor. Reviewers and skeptics confirmed these findings:\n\n${JSON.stringify(confirmed.map(f => ({ title: f.title, severity: f.severity, mustFix: f.verdict.mustFix, file: f.file, line: f.line, detail: f.detail, evidence: f.evidence, suggestedFix: f.suggestedFix, skepticReasoning: f.verdict.reasoning })), null, 2)}\n\nFix every mustFix finding. Fix the others too when cheap and clearly an improvement; otherwise list them as deviations with a reason. Never change behaviour or test assertions to make a finding go away. Commit (refactor(<module>): <why> + trailer), run the full gate, report.`,
    { label: `fix r${round}`, phase: 'Fix', schema: STEP_SCHEMA },
  )
  fixes.push(fix)
  if (!fix || !fix.gatePassed) {
    log('Fix round did not finish green; stopping for the orchestrator.')
    break
  }
  if (!mustFix.length) break
}

return {
  stepResults,
  confirmedFindings: allConfirmed.map(f => ({ lens: f.lens, title: f.title, severity: f.severity, mustFix: f.verdict.mustFix, file: f.file, detail: f.detail })),
  fixes,
}
