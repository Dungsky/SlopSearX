# Temporary A06 source and qualification tooling

These commands are migration tooling only. They invoke actual source adapter
methods with private HTTPX fixtures; no provider is contacted. Retain captured
JSON after G08 retires the seven exporters. The eight invocations are distinct:

```sh
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a06/oracle/export_source.py
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a06/oracle/export_source.py --regressions
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a06/oracle/export_shared.py
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a06/oracle/export_xml.py
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a06/oracle/export_xml_grammar.py
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a06/oracle/export_xml_stress.py
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a06/oracle/export_wire.py
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a06/oracle/export_json_matrix.py
```

The first four captures preserve139 historical records. Grammar adds165 actual
whole-source records. Stress adds five complete outputs and three explicit
large-field summaries, with deterministic recipes/input hashes; it does not
claim eight complete outputs. Parser replay consumes only applicable records.

`scripts/rust-a06-native-qualification.py` is an eighth temporary Python file. It
compiles an offline native probe, checks real inherited-file/socket FD closure,
stdio, resource limits and GNU absolute address-space behavior. It is currently a
required CI driver, explicitly inventoried as tooling, and needs a native
successor before G08 removes required Python. No backend runtime calls it.

Wire supplement captures63 complete request controls, four complete UTF7/ET error
controls, and every Unicode codepoint of the pinned Python word predicate. Native
query helpers replay169 first-request URLs and nine exact OpenAlex preflight
errors across historical/new observations; adapter lifecycle remains pending.

JSON-operation supplement adds198 complete actual OpenAlex/SemanticScholar
outputs, including143 source successes and55 classified source errors. These
are source controls, not native implementation or fixture-count acceptance.
