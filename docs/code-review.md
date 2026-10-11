# Pull request review operations

SlopSearX’s PR analysis calls a pinned reusable workflow in [`groktopus/codereview`](https://github.com/groktopus/codereview). The analysis workflow has read-only repository permissions. A separate protected publisher validates its artifact before it can submit a GitHub review using the target workflow’s `GITHUB_TOKEN`.

The deployed target-side v5 policy uses non-binding comment mode. The publisher invokes a JavaScript action pinned to the same immutable harness revision as the separately checked-out Python runtime. Under v5, a valid analysis result can retain its original `APPROVE` or `REQUEST_CHANGES` disposition in the result and comment text while the GitHub effect remains a `COMMENT`; the comment is not an approval, a change request, or merge authorization. The analysis verdict itself is not rewritten. Post-merge checks and actual v5 publication, replay, and containment verification remain pending. Verify the installed policy and runtime bindings before relying on a run. Droid Auto Review remains active; the separate Droid Tag and Wiki Refresh workflows are outside this rollout. No workflow in this process merges a pull request.

## Configure the reviewer endpoints

In the SlopSearX repository, open **Settings → Secrets and variables → Actions**. The workflow uses these six repository settings:

| Setting | Purpose |
|---|---|
| `LLM_BASE_URL` | LLM provider endpoint |
| `LLM_MODEL` | LLM reviewer model identifier |
| `LLM_API_KEY` | Credential for the LLM endpoint |
| `JEV_BASE_URL` | Bounded classifier endpoint |
| `JEV_MODEL` | Bounded classifier model identifier |
| `JEV_API_KEY` | Credential for the bounded classifier endpoint |

Use the approved endpoint and model values for this deployment. The bounded classifier supplies an advisory classification; deterministic runtime checks control coverage, allowed dispositions, duplicate protection, and publication. Store credentials only in Actions secrets. Never place secret values in workflow files, variables, pull requests, logs, or review artifacts. The central reusable workflow receives the provider settings, not the target repository’s GitHub write token.

## Artifact and publisher behavior

The publisher binds the artifact to its authenticated source run and artifact ID. For a download, it uses the exact temporary `Location` returned by the authenticated GitHub artifact API. It sends no GitHub bearer token to storage, follows no second redirect, and does not persist or log the signed location. It also checks the API artifact digest, bounded archive size, manifest identity, and source-run bindings. Do not add guessed storage hosts or wildcard allowances.

The v5 publisher calls a JavaScript action pinned to the same immutable harness revision as the separately checked-out Python runtime. The action receives the GitHub Actions artifact-runtime environment and launches only the fixed protected Python module from the pinned source tree. Keep the publisher on that action path: running the Python module directly in a shell step does not provide the artifact-runtime variables required for official artifact upload/readback. The workflow checks out protected publisher code and the pinned harness, never the pull-request head.

Earlier v2 canary attempts ended with publication `UNKNOWN` after static host checks rejected `productionresultssa9.blob.core.windows.net` and `productionresultssa14.blob.core.windows.net`. The first analysis reported `REQUEST_CHANGES` with `PARTIAL` coverage; the second reported `INCOMPLETE` with `PARTIAL` coverage and zero findings. These are historical outcomes, not evidence that a review was published or that later runs will have the same storage location.

The v4 policy includes four bounded historical reconciliation records for specific previously observed publisher runs. They are exact, run-bound exceptions; they do not waive history checks for other runs or turn an old `UNKNOWN` into a successful publication. The v5 policy carries those same records and leaves the ordinary bounded history scan subject to its existing limits.

The publisher can also recognize a previously posted review for the same validated pull-request slot, head, disposition, body, and actor. When the complete review listing proves that exact existing review, the publisher may report the effect as confirmed without posting another review. This is replay evidence about an existing review, not proof that the current attempt submitted it.

An acknowledged no-effect receipt means only that the identified publisher attempt did not invoke review submission. It is emitted only when the current publisher identity and attempt are verified and the official artifact upload is acknowledged. A missing acknowledgement, unverifiable identity or attempt, timeout, cancellation, or incomplete history remains `UNKNOWN`; do not infer that no review exists from an absent receipt. New submissions still require complete bounded history and the normal intent and readback checks.

## Read a run’s evidence

Open the **PR Review Analysis** run in Actions and inspect its result and provenance artifacts. Then inspect the corresponding protected publisher run and its receipt. Confirm the source run, artifact, repository, pull request, and head bindings agree. Read the disposition and coverage state, and check the pull request’s review timeline to establish whether an effect exists. Keep `PARTIAL`, `UNKNOWN`, and missing evidence visible. A green workflow, a disposition, or a published comment does not establish complete coverage or semantic review quality.

Earlier runs remain distinct evidence: the v4 PR764 publisher ended `UNKNOWN` because its no-effect receipt was not acknowledged; the v4 PR767 source result was `APPROVE` with `COMPLETE` coverage, while the strict comment-only publisher rejected it. Neither outcome was a confirmed review effect. The deployed v5 mode preserves the source disposition while publishing only a clearly labeled non-binding comment. Actual v5 publication, replay, and containment verification are still pending.

The available runs show that the workflow can analyze real pull requests. They do not establish calibrated review quality or accuracy. Treat model output as advisory until representative outcomes have been independently reviewed.

## Rollback

To stop future review effects, first disable only **PR Review Analysis** and the protected publisher workflow in the repository’s Actions settings. Droid Auto Review remains active throughout the canary and should not be changed during this rollback; the separate Droid Tag and Droid Wiki workflows are also outside its scope. Then use a scoped pull request to restore the prior publisher policy and job guards, checking the diff against intervening work. Keep the two review workflows disabled until restoration is complete. Rollback does not remove reviews already posted to pull requests.
