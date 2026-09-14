# Adversarial Verification of Your Own Claims

Your own verification is **not** independent verification. When a change rests
on factual claims — "this flag does not exist", "no occurrences remain", "the
version markers now match" — dispatch a subagent with **no shared context** to
attack those claims. Fresh context has no stake in the answer, and finds what
you talked yourself out of looking at.

Do this **before** marking the PR ready, not after merge.

## Dispatch it on the CLAIMS, not the diff

Give it the assertions, not your patch, so it cannot grade its own work:

```
delegate_task(
  goal="""Adversarial verification. The agent that made these changes asserts the
  following. Try to PROVE EACH ONE WRONG. Do not trust the descriptions — check
  the real artifact (run the binary, read the source, grep the repo).

  For EACH claim report: CONFIRMED / FALSE / INCOMPLETE, plus the exact command
  run and its output. Also hunt for: (a) other places the corrected string still
  appears that the fix missed; (b) NEW false statements the changes introduced.

  Claims:
  1. <claim>
  2. <claim>
  """,
  context="Read-only verification of <repo> at <revision>. Do not modify files."
)
```

Asking for CONFIRMED/FALSE/INCOMPLETE per claim is what makes the output
actionable rather than a wall of prose — and naming the exact command forces
the check to be reproducible instead of merely asserted.

## It earns its cost

An adversarial pass over a "finished" documentation fix returned 12 findings,
including **three errors in the fix itself**:

- a sensor that could never fire (asserted on the wrong command's output)
- a matcher too narrowly anchored to catch the defect stated in prose
- a factual claim about flag ordering that was simply wrong

It also reported the *unfinished* work honestly ("claim N is correct as an edit
but incomplete in coverage"), which is the signal to keep going rather than ship.

Expect noise. That is the point — it is a lead generator, not an oracle.

## Handling the output

1. **Verify every finding yourself before acting.** Treat findings as leads, not
   facts. Some will be wrong or mis-framed; a wrong "fix" applied on its say-so
   is a new defect. Reproduce each one against the artifact first.
2. **Freeze the revision.** The reviewer reads whatever HEAD is when it looks.
   If you keep pushing mid-review it reports on a moving target and part of the
   report is already stale. Either stop pushing until it reports, or tell it the
   exact revision (`git rev-parse --short HEAD`) and have it record which
   revision each verdict applies to.
3. **Stragglers go in a follow-up PR.** A long adversarial pass can report after
   the PR merged. Do not reopen; open a new PR that references the first and
   states explicitly which findings were already addressed.
4. **Attribute your own errors plainly.** When the review catches a mistake you
   made, give it its own heading in the PR body. It makes the review trail
   trustworthy and stops a future reader from re-deriving the same wrong belief.

## Budget it

Long adversarial passes are cheap in tokens but slow in wall-clock — one took
~20 minutes and 39 tool calls. Start it as soon as the claims are stable (right
after the implementation commit), so it finishes while you are still writing the
PR body and running gates, rather than after the merge.

## Scope the verifier: no builds, and fewer claims per pass

Two failure modes, both observed in one session, both fixable in the dispatch
prompt:

**1. Do not let the verifier run builds or the test suite.** It burns its budget
on work you are already doing — one verifier spent half of 42 calls on two
`cargo test` runs plus a `sleep 120`, then hit the wall-clock timeout while still
collecting data. Say it explicitly:

> Do NOT run cargo test / clippy / build (too slow) — verify with grep, sed, git,
> read_file only. The main agent already runs the suite.

**2. Keep a pass to ~5 claims.** A 12-claim pass timed out at 1500s with 42 calls
having produced all its evidence but no synthesis. The same material split into a
5-claim pass finished in 116s with 6 calls. If you have many claims, either run
two narrow passes or state that partial results are acceptable.

**When it still times out, the evidence is usually recoverable.** A timed-out
subagent leaves an append-only transcript with every tool result it collected —
read it and extract the verdicts rather than treating the pass as failed:

```bash
# tool results, in order, from a live transcript
grep -oP '^\d\d:\d\d:\d\d result\s+\| \K.*' \
  ~/.hermes/profiles/<profile>/cache/delegation/live/<deleg_id>/task-0.log
```

Then re-dispatch only the items still unverified, with a narrower scope. This is
recovery, not a substitute: a timeout near the start of a pass means you still
know nothing, and the findings must be established some other way.

**Check the artifact, not your intent.** One pass reported that a paper cited in
prose was absent from the reference list, and that a statistic had been
paraphrased into something the source does not say ("co-occur in the top-k of
>60%" vs. the paper's "over 60% of queries containing at least one highly
distracting passage among the top-10"). Both were real. The second is the same
defect class the change was written to fix — describing a source without
re-reading it — so a verifier that can catch it is worth its wall-clock.
