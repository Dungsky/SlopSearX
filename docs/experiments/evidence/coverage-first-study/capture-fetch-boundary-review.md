# Capture fetch-boundary review

Status: the original source review found an incomplete caller-side guard. A
subsequent isolated protected-capture lab has passed its declared boundary
probes for the current candidate runtime. This qualifies that setup profile; it
does not admit a coverage-study stage or authorize its source capture.

The original review examined candidate revision
`b5c1bf473bec78f3ebbfb630213b08612b3264fb`. It found an initial hostname/IP
guard, but did not establish restrictions at every outbound connection.
Adapter dispatch preceded the normal scrape-path check; HTTP clients could
resolve and follow redirects independently; browser and FlareSolverr fetching
delegated additional requests. The guard alone did not prove restrictions on
redirects, DNS changes, or browser subresources.

A later protected-capture lab records status
`setup_checks_passed_not_study_admission` for candidate agent revision
`15af8db803a049a67cb24095e037bc07a5bab75c`. The public result is pinned at
[commit f0c6b13cb68fe1b810c963265f74389a56af3460](https://github.com/magnus919/groktocrawl-x/blob/f0c6b13cb68fe1b810c963265f74389a56af3460/docs/experiments/evidence/protected-capture-lab-2026-10-11/result.json)
(SHA-256 `f950bb10c5b1d7c7e2a4a7d78e531e91fd5047d0a8f52618245c6232583931ba`).
Its [lab report](https://github.com/magnus919/groktocrawl-x/blob/f0c6b13cb68fe1b810c963265f74389a56af3460/docs/experiments/evidence/protected-capture-lab-2026-10-11/README.md)
records ten HTTPS checks and six declared protected-capture probes passing,
with the sentinel reporting zero unwanted requests and one deliberate positive
control. It also records one bounded model diagnostic; upstream usage was
unknown. The result explicitly reports zero scientific evaluation calls and
retains prior failures without repair or scientific credit.

This result supports the declared lab boundary profile for the recorded source,
images, configuration, and probes. It is not a general guarantee for arbitrary
configurations or later runtime changes. It does not replace the registered
coverage protocol's stage-bound source-capture permit, acquisition data, or
independent stage admission. A new runtime identity or changed boundary requires
its own review and evidence. The fixed study gates and no-retry policy remain
unchanged.

The original source pointers remain useful for understanding the guard that was
reviewed:

- [Main scrape dispatch and initial guard](https://github.com/magnus919/groktocrawl-x/blob/b5c1bf473bec78f3ebbfb630213b08612b3264fb/scraper-svc/scraper/fetch.py#L420)
- [Shared hostname/IP guard](https://github.com/magnus919/groktocrawl-x/blob/b5c1bf473bec78f3ebbfb630213b08612b3264fb/common/url.py#L221)
- [HTTP and browser fetching](https://github.com/magnus919/groktocrawl-x/blob/b5c1bf473bec78f3ebbfb630213b08612b3264fb/scraper-svc/scraper/fetch_tiers.py)
- [Adapter shared page fetch](https://github.com/magnus919/groktocrawl-x/blob/b5c1bf473bec78f3ebbfb630213b08612b3264fb/scraper-svc/scraper/adapters/_helpers.py#L14)
