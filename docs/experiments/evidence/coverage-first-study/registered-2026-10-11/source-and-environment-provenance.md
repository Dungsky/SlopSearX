# Source and environment provenance

The source revision is `9bb0b4edf9a07e5c558b8c9935f677d0fcba7ee3`, including the selector-permit batch receipt fix. This supersedes the uninvoked source revision recorded in commits `c25a6214a066d7b808a872f2ac7ab2cd23ba6f01` and `76c3b53328a2a6399a0477a1113ad8b86182d1af`; no stage was invoked on those pins, and the fix is included on `main` through PR #773 merge commit `282cf846e0f834aa55487f948ce66c1b8426a954`. The exact three-material qualified-source closure covers the coverage compiler, production reranker, and lock. Its byte pins remain verified against this source revision.

The sanitized target-host record is `target-host-preflight.json`, SHA-256 `0f31b9ab46edd291da11b53917b7cb730a8df902709c65b1c599870cd12ef7c4`. It reports Linux CPython 3.12.3, 135 locked distributions, and native CLI `codex-cli 0.162.1`. It records the requested model in the CLI catalog, zero configured MCP servers, and disabled stable environment tools. The current field is `scientific_provider_dispatches: 0`. The catalog metadata transport is unmetered/unknown; model-route qualification is not established.

The designated W0 preflight is `designated-w0-preflight.json` and is separately hash-pinned in `registration-inventory.json`. Neither preflight artifact is part of the 11 stage registration pins. The original private environment receipt is not included. A prior Mac-local source binding is also omitted; only its SHA-256 and `artifact_included: false` marker remain in the inventory.

These artifacts establish static source/environment identity only. They do not assert full stage preflight, stage admission, execution permission, protected-capture authority, or scientific results.
