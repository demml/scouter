import sys

from opentelemetry import trace


def refresh_google_adk_cached_tracers() -> None:
    """Point already-imported ADK test modules at the current OTel provider.

    ADK caches a module-level tracer during import. These integration tests
    intentionally tear down and recreate Scouter's global provider in one
    pytest process, so the imported ADK modules can otherwise keep a tracer
    from a provider that a previous test shut down.
    """
    adk_tracing = sys.modules.get("google.adk.telemetry.tracing")
    if adk_tracing is None:
        return

    old_tracer = getattr(adk_tracing, "tracer", None)
    new_tracer = trace.get_tracer(
        instrumenting_module_name="gcp.vertex.agent",
        instrumenting_library_version=adk_tracing.version.__version__,
        schema_url=adk_tracing.Schemas.V1_36_0.value,
    )
    setattr(adk_tracing, "tracer", new_tracer)

    for module_name, module in list(sys.modules.items()):
        if module_name.startswith("google.adk.") and getattr(module, "tracer", None) is old_tracer:
            setattr(module, "tracer", new_tracer)
