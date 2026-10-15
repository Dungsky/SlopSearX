#!/usr/bin/env python3
"""Re-export deterministic F04 observations from pinned Python source offline."""

from __future__ import annotations

import argparse
import asyncio
import hashlib
import importlib.util
import ipaddress
import json
import os
import socket
import subprocess
import sys
from dataclasses import asdict
from pathlib import Path
from typing import Any
from urllib.parse import urlsplit

import httpx
from fastapi.testclient import TestClient

ROOT = Path(__file__).resolve().parents[4]
SOURCE_ROOT = ROOT
BASELINE = "7b2d876307189d7a62000d1f289b3ef4b65eaff6"
ORACLE_COMMAND = (
    ".venv/bin/python docs/rust-rewrite/replay/oracle/export.py "
    "--source-root /private/tmp/slopsearx-f04-oracle-src "
    f"--baseline {BASELINE}"
)
SOURCE_PATHS = [
    "tests/fixtures/upstream_contracts/npm.json",
    "tests/fixtures/upstream_contracts/crates.json",
    "tests/fixtures/upstream_contracts/openalex.json",
    "tests/test_upstream_contracts.py",
    "scripts/upstream_contracts.py",
    "engines/npm.py",
    "engines/crates.py",
    "engines/openalex.py",
    "slopsearx/adapter.py",
    "slopsearx/payload.py",
    "tests/fixtures/searxng_api_contract.json",
    "tests/test_searxng_api_contract.py",
    "tests/test_mcp_harness.py",
    "slopsearx/mcp/harness.py",
    "slopsearx/server.py",
    "slopsearx/mcp/tools.py",
    "slopsearx/service.py",
    "slopsearx/snapshot.py",
]
ORIGIN = {
    "npm": "https://registry.npmjs.org",
    "crates": "https://crates.io",
    "openalex": "https://api.openalex.org",
}


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def git_blob(path: str) -> str:
    return subprocess.check_output(["git", "rev-parse", f"{BASELINE}:{path}"], cwd=SOURCE_ROOT, text=True).strip()


def install_loopback_network_guard() -> None:
    """Forbid name resolution and sockets to non-loopback hosts during export."""
    original_getaddrinfo = socket.getaddrinfo
    original_connect = socket.socket.connect
    original_connect_ex = socket.socket.connect_ex

    def guarded_getaddrinfo(host: Any, *args: Any, **kwargs: Any) -> Any:
        flags = kwargs.get("flags", args[4] if len(args) > 4 else 0)
        if flags & getattr(socket, "AI_NUMERICHOST", 0):
            return original_getaddrinfo(host, *args, **kwargs)
        try:
            address = ipaddress.ip_address(host)
        except (ValueError, TypeError):
            raise OSError("oracle export permits loopback addresses only")
        if not address.is_loopback:
            raise OSError("oracle export permits loopback addresses only")
        return original_getaddrinfo(host, *args, **kwargs)

    def guarded_connect(sock: socket.socket, address: Any) -> Any:
        if isinstance(address, tuple) and not ipaddress.ip_address(address[0]).is_loopback:
            raise OSError("oracle export permits loopback addresses only")
        return original_connect(sock, address)

    def guarded_connect_ex(sock: socket.socket, address: Any) -> Any:
        if isinstance(address, tuple) and not ipaddress.ip_address(address[0]).is_loopback:
            raise OSError("oracle export permits loopback addresses only")
        return original_connect_ex(sock, address)

    socket.getaddrinfo = guarded_getaddrinfo
    socket.socket.connect = guarded_connect
    socket.socket.connect_ex = guarded_connect_ex


def adapter_case(name: str) -> dict[str, Any]:
    from engines.crates import CratesAdapter
    from engines.npm import NpmAdapter
    from engines.openalex import OpenAlexAdapter

    adapter_class = {"npm": NpmAdapter, "crates": CratesAdapter, "openalex": OpenAlexAdapter}[name]
    fixture = load_json(SOURCE_ROOT / "tests/fixtures/upstream_contracts" / f"{name}.json")
    response_body = json.dumps(fixture["payload"], separators=(",", ":"), ensure_ascii=False)
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return httpx.Response(200, content=response_body, headers={"content-type": "application/json"})

    config: dict[str, Any] = {"max_results": 3, "timeout_ms": 5000}
    if name == "npm":
        config["base_url"] = "https://registry.npmjs.org/-/v1/search"
    elif name == "crates":
        config["base_url"] = "https://crates.io/api/v1/crates"
    else:
        config["base_url"] = "https://api.openalex.org"

    async def invoke() -> tuple[dict[str, Any], httpx.Request]:
        adapter = adapter_class(config=config)
        adapter.http_client = lambda **kwargs: httpx.AsyncClient(
            transport=httpx.MockTransport(handler), follow_redirects=False, trust_env=False, **kwargs
        )
        response = await adapter.search(fixture["query"])
        output = {
            "status": response.status.value,
            "body": {
                "results": [{**asdict(result), "engines": sorted(result.engines)} for result in response.results],
                "latency_ms": 0.0,  # explicitly volatile; canonicalized for byte-stable exports
                "error_message": response.error_message,
            },
        }
        return output, requests[-1]

    first, request = asyncio.run(invoke())
    second, _ = asyncio.run(invoke())
    parsed = urlsplit(str(request.url))
    target = parsed.path + (f"?{parsed.query}" if parsed.query else "")
    headers = {
        key.lower(): value
        for key, value in request.headers.items()
        if key.lower() not in {"host", "connection", "content-length"}
    }
    exchange = {
        "request": {"method": request.method, "target": target, "headers": headers, "body": ""},
        "response": {
            "status": 200,
            "headers": {"content-type": "application/json"},
            "body": response_body,
        },
    }
    return {
        "id": f"upstream-{name}-captured",
        "kind": "adapter_http",
        "input": {"engine": name, "query": fixture["query"], "params": {}, "max_results": 3},
        "upstream_origin": ORIGIN[name],
        "clock": {"unix_seconds": 1792022400, "ids": [f"fixture-{name}-001"]},
        "exchanges": [exchange],
        "expected": first,
        "candidate": second,
        "volatility": ["/body/latency_ms"],
        "oracle": {
            "command": ORACLE_COMMAND,
            "baseline_commit": BASELINE,
            "observation": (
                "Two independent pinned Python adapter executions against this sanitized "
                "recorded response; not Rust parity evidence."
            ),
        },
    }


def import_contract_engine() -> type:
    path = SOURCE_ROOT / "tests/test_searxng_api_contract.py"
    spec = importlib.util.spec_from_file_location("f04_searxng_contract", path)
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load pinned HTTP contract source")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module._ContractEngine


def search_http_observation() -> dict[str, Any]:
    import slopsearx.server as server_module
    import slopsearx.service as service

    engine_class = import_contract_engine()
    server_module._active_engines = {"contract": engine_class()}
    server_module._router = None
    service.generate_query_id = lambda: "ssx-f04-search-http"
    with TestClient(server_module.app) as client:
        response = client.get("/search", params={"q": "F04 contract", "format": "json", "engines": "contract"})
    body = response.json()
    body["meta"]["response_time_ms"] = 0  # explicitly volatile; canonicalized for byte-stable exports
    return {"status": response.status_code, "body": body}


def search_http_case() -> dict[str, Any]:
    expected = search_http_observation()
    candidate = search_http_observation()
    return {
        "id": "search-http-json-contract",
        "kind": "search_http",
        "input": {
            "method": "GET",
            "path": "/search",
            "params": {"q": "F04 contract", "format": "json", "engines": "contract"},
        },
        "upstream_origin": None,
        "clock": {"unix_seconds": 1792022400, "ids": ["ssx-f04-search-http"]},
        "exchanges": [],
        "expected": expected,
        "candidate": candidate,
        "volatility": ["/body/meta/response_time_ms"],
        "oracle": {
            "command": ORACLE_COMMAND,
            "baseline_commit": BASELINE,
            "observation": (
                "Two independent FastAPI TestClient executions using the pinned "
                "deterministic contract engine; no upstream traffic."
            ),
        },
    }


async def _mcp_observation() -> dict[str, Any]:
    import uvicorn
    from mcp import ClientSession
    from mcp.client.streamable_http import streamable_http_client

    import slopsearx.service as service
    import slopsearx.snapshot as snapshot_module
    from slopsearx.mcp.harness import FakeEngineSpec, make_fixture_http_app

    service.generate_query_id = lambda: "ssx-f04-mcp-search"
    import types

    original_uuid4 = snapshot_module.uuid.uuid4
    original_time = snapshot_module.time.time
    snapshot_module.uuid.uuid4 = lambda: types.SimpleNamespace(hex="f04fixedsnapshot")
    snapshot_module.time.time = lambda: 1792022400.0
    app = make_fixture_http_app([FakeEngineSpec(name="wikipedia", count=2)])
    server = uvicorn.Server(uvicorn.Config(app, host="127.0.0.1", port=0, log_level="error"))
    task = asyncio.create_task(server.serve())
    try:
        async with asyncio.timeout(10):
            while not server.started:
                if task.done():
                    await task
                    raise RuntimeError("MCP fixture server exited before startup")
                await asyncio.sleep(0.01)
        port = server.servers[0].sockets[0].getsockname()[1]
        observed_statuses: list[int] = []

        async def record_response(response: httpx.Response) -> None:
            observed_statuses.append(response.status_code)

        async with httpx.AsyncClient(event_hooks={"response": [record_response]}) as client:
            async with streamable_http_client(f"http://127.0.0.1:{port}/mcp", http_client=client) as streams:
                async with ClientSession(streams[0], streams[1]) as session:
                    await session.initialize()
                    result = await session.call_tool(
                        "slopsearx_search",
                        {
                            "query": "F04 MCP contract",
                            "engines": ["wikipedia"],
                            "language": "de",
                            "time_range": "week",
                            "safesearch": "moderate",
                        },
                    )
                    if not observed_statuses:
                        raise RuntimeError("MCP client produced no HTTP response")
                    body = json.loads(result.content[0].text)
                    body["meta"]["response_time_ms"] = 0  # explicitly volatile; canonicalized for byte-stable exports
                    return {"status": observed_statuses[-1], "body": body}
    finally:
        snapshot_module.uuid.uuid4 = original_uuid4
        snapshot_module.time.time = original_time
        server.should_exit = True
        try:
            await asyncio.wait_for(task, timeout=10)
        except TimeoutError:
            task.cancel()
            try:
                await task
            except asyncio.CancelledError:
                pass


def mcp_search_observation() -> dict[str, Any]:
    return asyncio.run(_mcp_observation())


def mcp_search_case() -> dict[str, Any]:
    expected = mcp_search_observation()
    candidate = mcp_search_observation()
    return {
        "id": "mcp-search-filter-enforcement",
        "kind": "mcp_search",
        "input": {
            "tool": "slopsearx_search",
            "query": "F04 MCP contract",
            "engines": ["wikipedia"],
            "language": "de",
            "time_range": "week",
            "safesearch": "moderate",
        },
        "upstream_origin": None,
        "clock": {"unix_seconds": 1792022400, "ids": ["ssx-f04-mcp-search"]},
        "exchanges": [],
        "expected": expected,
        "candidate": candidate,
        "volatility": ["/body/meta/response_time_ms"],
        "oracle": {
            "command": ORACLE_COMMAND,
            "baseline_commit": BASELINE,
            "observation": (
                "Two independent calls over the real MCP streamable-HTTP surface with "
                "FakeEngine and InMemoryStore; no upstream traffic."
            ),
        },
    }


def run_adapter_candidate(input_value: dict[str, Any]) -> dict[str, Any]:
    from engines.crates import CratesAdapter
    from engines.npm import NpmAdapter
    from engines.openalex import OpenAlexAdapter

    name = input_value["engine"]
    adapter_class = {"npm": NpmAdapter, "crates": CratesAdapter, "openalex": OpenAlexAdapter}[name]
    origin = os.environ["SSX_REPLAY_URL"]
    path = {"npm": "/-/v1/search", "crates": "/api/v1/crates", "openalex": "/works"}[name]
    config = {"max_results": input_value["max_results"], "timeout_ms": 5000}
    config["base_url"] = origin + ("" if name == "openalex" else path)
    if name == "openalex":
        config["base_url"] = origin

    async def run() -> dict[str, Any]:
        adapter = adapter_class(config=config)
        response = await adapter.search(input_value["query"], input_value.get("params"))
        return {
            "status": response.status.value,
            "body": {
                "results": [{**asdict(row), "engines": sorted(row.engines)} for row in response.results],
                "latency_ms": response.latency_ms,
                "error_message": response.error_message,
            },
        }

    return asyncio.run(run())


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", required=True)
    parser.add_argument("--candidate", action="store_true", help="emit one observation for cargo xtask replay-run")
    parser.add_argument("--case", help="case ID for --candidate mode")
    parser.add_argument(
        "--source-root", type=Path, required=True, help="clean detached checkout at the pinned Python oracle commit"
    )
    parser.add_argument("--output", default="crates/xtask/fixtures/replay/corpus.json")
    args = parser.parse_args()
    global SOURCE_ROOT
    if args.baseline != BASELINE:
        raise SystemExit(f"baseline must be the pinned F04 source revision {BASELINE}")
    SOURCE_ROOT = args.source_root.resolve()
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=SOURCE_ROOT, text=True).strip()
    dirty = subprocess.check_output(
        ["git", "status", "--porcelain", "--untracked-files=all"], cwd=SOURCE_ROOT, text=True
    )
    if commit != BASELINE or dirty:
        raise SystemExit(f"oracle source root must be clean at {BASELINE}; got {commit}, dirty={bool(dirty)}")
    sys.path.insert(0, str(SOURCE_ROOT))
    package_spec = importlib.util.find_spec("slopsearx")
    if (
        package_spec is None
        or package_spec.origin is None
        or not Path(package_spec.origin).resolve().is_relative_to(SOURCE_ROOT)
    ):
        raise SystemExit("Python package import does not resolve from the pinned source root")
    engines_spec = importlib.util.find_spec("engines")
    if (
        engines_spec is None
        or engines_spec.origin is None
        or not Path(engines_spec.origin).resolve().is_relative_to(SOURCE_ROOT)
    ):
        raise SystemExit("engine imports do not resolve from the pinned source root")
    sys.dont_write_bytecode = True
    for key in list(os.environ):
        if key.startswith(("ENGINE_", "MCP_", "JEV_")) or key.lower() in {"http_proxy", "https_proxy", "all_proxy"}:
            os.environ.pop(key, None)
    install_loopback_network_guard()
    if args.candidate:
        if args.case not in {"upstream-npm-captured", "upstream-crates-captured", "upstream-openalex-captured"}:
            raise SystemExit("--candidate currently supports adapter_http cases only")
        observation = run_adapter_candidate(json.loads(os.environ["SSX_REPLAY_INPUT"]))
        print(json.dumps(observation, separators=(",", ":")))
        return
    provenance = {
        "license": (
            "Corpus structure and generated observations are MIT-licensed with this repository; "
            "embedded sanitized upstream API facts retain their source terms. This corpus does "
            "not assert a separate license grant for those facts."
        ),
        "redaction": (
            "Only existing allowlisted public parser fields are retained; no request/response "
            "identity headers, credentials, author/publisher identities, raw errors, or "
            "unsanitized provider payloads."
        ),
        "sources": [{"path": path, "git_blob": git_blob(path)} for path in SOURCE_PATHS],
        "oracle": {
            "path": "docs/rust-rewrite/replay/oracle/export.py",
            "sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "invocation": ORACLE_COMMAND,
        },
    }
    corpus = {
        "schema_version": 1,
        "baseline_commit": BASELINE,
        "provenance": provenance,
        "cases": [
            *(adapter_case(name) for name in ("npm", "crates", "openalex")),
            search_http_case(),
            mcp_search_case(),
        ],
    }
    output = ROOT / args.output
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(corpus, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"wrote {len(corpus['cases'])} cases to {output.relative_to(ROOT)} from Python source at {commit}")


if __name__ == "__main__":
    main()
