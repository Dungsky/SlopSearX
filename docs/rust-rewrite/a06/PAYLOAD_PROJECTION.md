# Payload-only source projection

The additive `adapters::source_payload::project_field` implements the source
`_json_safe` operation for one retained top-level `build_payload` data field.
Its initial depth is1: the compact data dictionary is depth0. At depth>100,
any value becomes `<max depth exceeded>` before scalar or container processing.

Each UTF16 surrogate codepoint becomes one U+FFFD in both values and nested
object keys. Raw adjacent surrogate codepoints are not recombined; a scalar
emoji stays a scalar emoji. Key collisions after sanitizing preserve the first
insertion position and final value, matching Python's dictionary comprehension.
Nested nulls, false, zero, empty containers and order remain intact. The caller
drops only top-level None data values before this operation and constructs
provenance from the original retained field names in their original order.

Ordinary `Value::project`, result title/content, JSON parsing and text storage
remain unchanged. Nonfinite numbers still return an explicit typed projection
refusal; they are never silently replaced with null. Source `_json_safe` itself
retains nonfinite floats. Consumers must preserve the existing approved native
finite-output boundary and finish source operations before applying a pending
typed refusal. This component does not resolve that boundary globally.

The seven controls derive from actual pinned CPython3.14.7 `build_payload`
execution captured by A07. `source-payload-controls.json` records source hashes,
codepoint-tagged inputs for the four Unicode/order cases, exact list-depth
recipes for three depth cases, full expected data JSON and provenance field
order. It binds the original supplemental packet SHA256
`fe7593e1c308f1c64f3b1ac261d256058a6cabc77f63fa5ae378b2648ae50ebc`.
They are direct source helper controls, not seven complete adapter observations.

This is a semantic component with fallible reservations, not aggregate allocation
or cooperative CPU/deadline qualification. Strings are counted then copied;
arrays/maps are copied recursively through the fixed source depth boundary.
Runtime consumption requires supervised/charged ownership already being built
for A06, or separately qualified bounded ownership. No uncharged global clone,
factory activation, GNU qualification or retained-result memory claim follows.
