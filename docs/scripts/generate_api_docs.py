#!/usr/bin/env python3
"""Generate the Starlight API reference page from Scouter's Python exports."""

from __future__ import annotations

import ast
import re
from collections import OrderedDict
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
INIT_FILE = REPO_ROOT / "py-scouter/python/scouter/__init__.py"
STUB_FILE = REPO_ROOT / "py-scouter/python/scouter/_scouter.pyi"
OUTPUT_FILE = REPO_ROOT / "docs/src/content/docs/api/index.md"

MODULE_SUMMARIES = {
    "agent": "Provider integrations, prompt types, and LLM-facing helpers.",
    "alert": "Alert configuration objects for Slack, OpsGenie, and console dispatch.",
    "bifrost": "Data access helpers for table-oriented reads and writes.",
    "client": "HTTP client helpers for registering profiles and querying results.",
    "drift": "Drift profile creation and drift result types.",
    "evaluate": "Offline and online evaluation helpers, task types, and result handling.",
    "logging": "Rust-backed logging configuration and logger access.",
    "mock": "Fixtures and mock utilities used in tests and examples.",
    "observe": "Observation helpers for runtime instrumentation.",
    "profile": "Data profiling classes and feature statistics.",
    "queue": "Queue producers and async ingestion helpers.",
    "service_map": "Service map middleware and connection records.",
    "trace": "Convenience wrappers for trace context utilities.",
    "tracing": "Tracing runtime, exporters, and instrumentors.",
    "transport": "Transport configuration for HTTP, gRPC, Kafka, RabbitMQ, and Redis.",
    "types": "Shared feature, metric, and alert types.",
    "util": "Utility helpers used by client-side integrations.",
}

CATEGORIES = OrderedDict(
    [
        (
            "Drift and profiling",
            {
                "Drifter",
                "DataProfiler",
                "DataProfile",
                "PsiDriftConfig",
                "PsiDriftProfile",
                "PsiDriftMap",
                "SpcDriftConfig",
                "SpcDriftProfile",
                "SpcFeatureDriftProfile",
                "SpcFeatureDrift",
                "SpcDriftMap",
                "CustomDriftProfile",
                "CustomMetric",
                "CustomMetricDriftConfig",
                "FeatureMap",
                "QuantileBinning",
                "EqualWidthBinning",
                "Manual",
                "SquareRoot",
                "Sturges",
                "Rice",
                "Doane",
                "Scott",
                "TerrellScott",
                "FreedmanDiaconis",
            },
        ),
        (
            "Queue and transport",
            {
                "ScouterQueue",
                "Queue",
                "ScouterClient",
                "DatasetClient",
                "DatasetProducer",
                "TableConfig",
                "WriteConfig",
                "HttpConfig",
                "GrpcConfig",
                "KafkaConfig",
                "RabbitMQConfig",
                "RedisConfig",
            },
        ),
        (
            "Evaluation",
            {
                "AgentEvalConfig",
                "AgentEvalProfile",
                "EvalRecord",
                "LLMJudgeTask",
                "AssertionTask",
                "ComparisonOperator",
                "EvalResults",
                "TraceAssertion",
                "TraceAssertionTask",
                "SpanStatus",
                "AggregationType",
                "SpanFilter",
            },
        ),
        (
            "Shared types and scheduling",
            {
                "Feature",
                "Features",
                "Metric",
                "Metrics",
                "CommonCrons",
                "PsiAlertConfig",
                "SpcAlertConfig",
                "CustomMetricAlertConfig",
                "Bifrost",
            },
        ),
        (
            "Service map",
            {
                "ServiceConnectionRecord",
                "ServiceMapMiddleware",
            },
        ),
    ]
)


def get_docstring_first_paragraph(node: ast.AST) -> str:
    docstring = ast.get_docstring(node) or ""
    paragraph = docstring.strip().split("\n\n", 1)[0].strip()
    return re.sub(r"\s+", " ", paragraph)


def get_signature(node: ast.ClassDef | ast.FunctionDef | ast.AsyncFunctionDef) -> str:
    if isinstance(node, ast.ClassDef):
        init_node = next(
            (
                child
                for child in node.body
                if isinstance(child, ast.FunctionDef) and child.name == "__init__"
            ),
            None,
        )
        if init_node is None:
            return f"class {node.name}"
        return f"{node.name}{format_args(init_node.args, drop_self=True)}"

    return f"{node.name}{format_args(node.args, drop_self=False)}"


def format_args(args: ast.arguments, *, drop_self: bool) -> str:
    positional = list(args.posonlyargs) + list(args.args)
    if drop_self and positional:
        positional = positional[1:]

    defaults = [None] * (len(positional) - len(args.defaults)) + list(args.defaults)
    parts: list[str] = []

    for arg, default in zip(positional, defaults):
        parts.append(format_arg(arg, default))

    if args.vararg:
        parts.append(f"*{args.vararg.arg}")
    elif args.kwonlyargs:
        parts.append("*")

    for arg, default in zip(args.kwonlyargs, args.kw_defaults):
        parts.append(format_arg(arg, default))

    if args.kwarg:
        parts.append(f"**{args.kwarg.arg}")

    return f"({', '.join(parts)})"


def format_arg(arg: ast.arg, default: ast.AST | None) -> str:
    text = arg.arg
    if arg.annotation is not None:
        text += f": {ast.unparse(arg.annotation)}"
    if default is not None:
        text += f" = {ast.unparse(default)}"
    return text


def parse_exports() -> tuple[list[str], list[str]]:
    tree = ast.parse(INIT_FILE.read_text(encoding="utf-8"))
    modules: list[str] = []
    exports: list[str] = []

    for node in tree.body:
        if isinstance(node, ast.Assign):
            for target in node.targets:
                if isinstance(target, ast.Name) and target.id == "__all__":
                    exports = [
                        elt.value
                        for elt in node.value.elts
                        if isinstance(elt, ast.Constant) and isinstance(elt.value, str)
                    ]

    for name in exports:
        if name in MODULE_SUMMARIES:
            modules.append(name)

    return modules, [name for name in exports if name not in MODULE_SUMMARIES]


def parse_stub_entries() -> dict[str, dict[str, object]]:
    tree = ast.parse(STUB_FILE.read_text(encoding="utf-8"))
    entries: dict[str, dict[str, object]] = {}

    for node in tree.body:
        if isinstance(node, (ast.ClassDef, ast.FunctionDef, ast.AsyncFunctionDef)):
            summary = get_docstring_first_paragraph(node)
            if isinstance(node, ast.ClassDef) and not summary:
                init_node = next(
                    (
                        child
                        for child in node.body
                        if isinstance(child, ast.FunctionDef) and child.name == "__init__"
                    ),
                    None,
                )
                if init_node is not None:
                    summary = get_docstring_first_paragraph(init_node)
            if not summary:
                summary = "See the signature above and the guide docs for usage examples."
            entries[node.name] = {
                "signature": get_signature(node),
                "summary": summary,
            }

    return entries


def build_page() -> str:
    modules, exports = parse_exports()
    stub_entries = parse_stub_entries()
    remaining = set(exports)

    lines = [
        "---",
        'title: "Python API reference"',
        'description: "Top-level Python exports available from the scouter package."',
        "---",
        "",
        "This page covers the public names exported from `scouter`.",
        "For end-to-end setup and examples, start with the guides in the rest of the docs.",
        "",
        "## Package modules",
        "",
    ]

    for module in modules:
        lines.append(f"- `{module}`: {MODULE_SUMMARIES[module]}")

    lines.extend(["", "## Top-level exports", ""])

    for section, names in CATEGORIES.items():
        section_names = [name for name in exports if name in names]
        if not section_names:
            continue

        lines.extend([f"### {section}", ""])
        for name in section_names:
            remaining.discard(name)
            entry = stub_entries.get(name, {})
            signature = entry.get("signature", name)
            summary = entry.get("summary", "No stub docstring was found for this export.")
            lines.append(f"#### `{name}`")
            lines.append("")
            lines.append(f"```python\n{signature}\n```")
            lines.append("")
            lines.append(summary)
            lines.append("")

    if remaining:
        lines.extend(["### Other exports", ""])
        for name in exports:
            if name not in remaining:
                continue
            entry = stub_entries.get(name, {})
            signature = entry.get("signature", name)
            summary = entry.get("summary", "No stub docstring was found for this export.")
            lines.append(f"#### `{name}`")
            lines.append("")
            lines.append(f"```python\n{signature}\n```")
            lines.append("")
            lines.append(summary)
            lines.append("")

    return "\n".join(lines).rstrip() + "\n"


def main() -> None:
    OUTPUT_FILE.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT_FILE.write_text(build_page(), encoding="utf-8")


if __name__ == "__main__":
    main()
