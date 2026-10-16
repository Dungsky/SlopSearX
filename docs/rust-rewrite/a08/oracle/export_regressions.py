"""Offline instrumentation of the actual old regression tests; retired G08."""

import contextvars
import hashlib
import json
import os
import pathlib
import sys
import unicodedata

import httpx
import pytest

ROOT = pathlib.Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT))
assert sys.version_info[:3] == (3, 14, 7) and unicodedata.unidata_version == "16.0.0"
from engines.edgar import EdgarAdapter  # noqa: E402
from engines.fred import FredAdapter  # noqa: E402
from engines.openfda import OpenFDAAdapter  # noqa: E402
from engines.oyez import OyezAdapter  # noqa: E402

classes = [EdgarAdapter, FredAdapter, OpenFDAAdapter, OyezAdapter]
current = contextvars.ContextVar("capture", default=None)
records = []
current_node = None
test_results = []


def clean(s, keys):
    for key in keys:
        if key and key.strip():
            s = s.replace(key, "<synthetic-key>")
    return s


def output(result):
    return dict(
        status=result.status.value,
        error_message=result.error_message,
        results=[
            {
                k: getattr(r, k)
                for k in [
                    "url",
                    "title",
                    "content",
                    "engine",
                    "position",
                    "score",
                    "category",
                    "published_date",
                    "thumbnail",
                    "img_src",
                    "media",
                    "payload",
                ]
            }
            for r in result.results
        ],
    )


class Plugin:
    def pytest_runtest_setup(self, item):
        global current_node
        current_node = item.nodeid

    def pytest_runtest_logreport(self, report):
        if report.when == "call":
            test_results.append(dict(node=report.nodeid, outcome=report.outcome))


original_send = httpx.AsyncClient.send


async def send(client, req, *args, **kwargs):
    case = current.get()
    if case is None:
        return await original_send(client, req, *args, **kwargs)
    keys = case.pop("_keys_tmp") if "_keys_tmp" in case else case["_keys"]
    case["_keys"] = keys
    case["requests"].append(
        dict(
            method=req.method,
            url=clean(str(req.url), keys),
            body=req.content.decode(),
            headers={k: v for k, v in req.headers.items() if k in ["user-agent", "accept", "content-type"]},
        )
    )
    try:
        response = await original_send(client, req, *args, **kwargs)
        case["responses"].append(dict(status=response.status_code, wire_body=response.content.decode()))
        return response
    except httpx.TimeoutException:
        case["responses"].append(dict(status=200, exception="timeout"))
        raise
    except Exception as exc:
        case["responses"].append(dict(status=200, exception=type(exc).__name__, diagnostic=clean(str(exc), keys)))
        raise


for cls in classes:
    original = cls.search

    # Keep the actual result in the wrapper itself rather than serializing/rebuilding.
    def make(original):
        async def search(adapter, query, params=None):
            cfg = dict(adapter.config)
            keys = [v for k, v in cfg.items() if "key" in k.lower() and isinstance(v, str)]
            env = os.environ.get("ENGINE_FRED_API_KEY")
            if env:
                keys.append(env)
            public = {
                k: ("synthetic-fred-key" if "key" in k.lower() and isinstance(v, str) and v.strip() else v)
                for k, v in cfg.items()
            }
            if env:
                public["environment_api_key"] = "synthetic-fred-key" if env.strip() else env
            case = dict(
                id=current_node + "#" + str(sum(r["node"] == current_node for r in records)),
                node=current_node,
                engine=adapter.name,
                query=query,
                config=public,
                responses=[],
                requests=[],
                _keys=keys,
            )
            token = current.set(case)
            try:
                result = await original(adapter, query, params)
                case["output"] = output(result)
                return result
            except Exception as exc:
                case["output"] = dict(raised=type(exc).__name__, error_message=clean(str(exc), keys))
                raise
            finally:
                current.reset(token)
                case.pop("_keys", None)
                records.append(case)

        return search

    cls.search = make(original)
httpx.AsyncClient.send = send
args = [
    "-q",
    "--no-cov",
    "-p",
    "no:cacheprovider",
    "tests/test_openfda.py",
    "tests/test_edgar_url_contract.py",
    "tests/test_fred.py",
    "tests/test_oyez_relevance.py",
    "tests/test_new_engines.py::TestOpenFDAAdapter",
    "tests/test_new_engines.py::TestEdgarAdapter",
    "tests/test_new_engines.py::TestFredAdapter",
    "tests/test_new_engines.py::TestOyezAdapter",
    "tests/test_payload.py::TestFinancialPayload::test_fred_payload",
    "tests/test_payload.py::TestBiomedicalPayload",
]
code = pytest.main(args, plugins=[Plugin()])
assert code == 0
p = ROOT / "docs/rust-rewrite/a08/regressions.json"
p.write_text(
    json.dumps(
        dict(
            schema_version=1,
            python=sys.version,
            unicode=unicodedata.unidata_version,
            producer_sha256=hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(),
            results=test_results,
            records=records,
        ),
        ensure_ascii=True,
        indent=2,
    )
    + "\n"
)
print(len(records), hashlib.sha256(p.read_bytes()).hexdigest())
