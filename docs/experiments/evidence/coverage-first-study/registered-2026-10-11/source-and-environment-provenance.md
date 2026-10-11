# Source and environment provenance

The source revision is `a3e2b81724e462fe72606e715112ede587cb07a2`. The exact three-material qualified-source closure covers the coverage compiler, production reranker, and lock. Its byte pins remain verified against this source revision.

The sanitized target-host record is `target-host-preflight.json`, SHA-256 `6f473c00ac9128bfba62403f760d7c4feaee44805ec5f7a8eab276c3f08c2ffe`. It reports Linux CPython 3.12.3, 135 locked distributions, and native CLI `codex-cli 0.162.1`. It records the requested model in the CLI catalog, zero configured MCP servers, and disabled stable environment tools. The current field is `scientific_provider_dispatches: 0`. The catalog metadata transport is unmetered/unknown; model-route qualification is not established.

The designated W0 preflight is `designated-w0-preflight.json` and is separately hash-pinned in `registration-inventory.json`. Neither preflight artifact is part of the 11 stage registration pins. The original private environment receipt is not included. A prior Mac-local source binding is also omitted; only its SHA-256 and `artifact_included: false` marker remain in the inventory.

These artifacts establish static source/environment identity only. They do not assert full stage preflight, stage admission, execution permission, protected-capture authority, or scientific results.
