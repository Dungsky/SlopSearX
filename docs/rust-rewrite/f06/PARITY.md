# F06 configuration and registry parity

The frozen source boundary is upstream `7cc420f7ffb61127750e77bf6130e225925f9186`; `source-pins.json` records Git blob identities for config, adapter metadata, the Jev catalog, all 51 engine modules, and the user-facing engine tables. `catalog.json` is the checked generated aggregate of the per-engine fragments. The runtime descriptor retains every current source while marking each native provider implementation unavailable until its adapter factory is added.

The native loader implements built-in defaults, partial YAML overlays, custom engine entries, `ENGINE_*` and `SEARCH_*` precedence, category replacement, HuggingFace's `HF_TOKEN` alias, Censys' two credentials, and secret-free debug/capability views. Censys `auth_configured` follows the existing catalog rule and checks its API key only. Environment parsing retains the current comma trimming and boolean-token behavior. Unknown top-level and search/routing fields are ignored; unrecognized cache/ranking values are retained as extension fields because the Python dataclasses accept them dynamically.

**Unresolved compatibility case:** Python `EngineEntry` accepts `api_key` and `api_secret` in YAML before applying environment overrides. Native configuration currently accepts credentials only from the environment. The source-backed `file_credentials` test requires YAML support plus environment precedence and redaction, so `cargo xtask check F06` must remain red until this case is resolved. The patch to restore YAML credential parsing was rejected by automatic approval review as a persistent secret-handling weakening. No intentional-difference decision has been approved; this is a blocking parity gap.

Typed Rust fields also reject some malformed YAML values at load time that Python stores dynamically and may fail on later use. Tests cover root scalars, invalid environment numerics and malformed YAML. Any additional type-level difference must be captured here with its source case and a reviewed compatibility decision before the gate is treated as acceptance evidence.

Routing roles are retrieval-fit metadata only. `routing-expectations.json` independently freezes each engine's role, purpose and `use_when` from `slopsearx/jev.py`, bound to the exact blob already recorded in `source-pins.json`. The F06 gate compares every named descriptor with that fixture and runs a same-count per-engine card-swap negative case. Aggregate role counts alone are insufficient. The catalog does not imply an adapter has been ported, has credentials, is enabled, or is eligible under current policy. `supported_filters` records adapter parameter consumption; `enforced_filters` is a separate layer-qualified declaration. The frozen source currently declares OpenAlex date bounds and local relative-time enforcement.

The environment layer uses accepted F05 numeric-text normalization for Python
signs, between-digit underscores, Unicode16 decimal digits and numeric whitespace.
This shared leaf only normalizes text; each caller still parses its numeric type
and applies existing bounds. SEARCH_CACHE_TTL_SECONDS remains strictly positive.
Engine integer fields retain the native i64 range; Python integers outside that
range fail native load rather than wrapping. Python str.strip for comma-separated
engine lists, categories and formats additionally removes U+001C–U+001F. Engine
lists preserve empty entries; categories/formats remove them. Boolean environment
values keep Python's lowercase-token rule without whitespace stripping. The
normalization algorithm itself is unchanged from accepted F05; domain never
imports config. These checks do not modify the unresolved credential case.

Numeric engine settings retain raw scalar type (integer, float, text or boolean).
The descriptor defaults retain JSON integer/float identity; YAML replacement
then determines environment coercion. Thus Brave's integer rate limit rejects a
fractional environment value, arXiv's float accepts one, and a None-default rate
limit retains its environment value as text. A raw value is not a validated
operational limit: downstream ports must use effective_i64/effective_f64 and
apply their own bounds. These accessors reject out-of-range/fractional integers
and nonfinite numeric values. Raw source-compatible nonfinite/text values are
not silently promoted into valid effective numbers. Container-valued engine
numeric fields and integers outside i64 fail native load as explicit type bounds.

SEARCH suffixes use source lowercase matching with the uppercase SEARCH_ prefix
required. Aliases are resolved before coercion; the last raw alias wins. Runtime
loading preserves environment iteration order, while injected BTreeMap input uses
its deterministic order. Engine suffix aliases follow the same source rule.
This does not trim boolean tokens or broaden the exact HF_TOKEN credential alias
precedence. Explicit YAML credential parsing and its failing regression remain
unchanged and approval-blocked.
