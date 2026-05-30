/**
 * Item 2: Frontend Browser RUM — OpenTelemetry Web Instrumentation
 *
 * Initializes a WebTracerProvider with:
 *   - service.name = zentra-frontend
 *   - OTLP HTTP exporter (traces)
 *   - FetchInstrumentation with traceparent propagation
 *   - DocumentLoad + UserInteraction instrumentation
 *   - W3C TraceContext propagator
 *
 * Import this file FIRST in main.tsx before any other code.
 * Uses dynamic imports so the build succeeds even without @opentelemetry packages.
 */

const OTEL_ENDPOINT = import.meta.env.VITE_OTEL_EXPORTER_OTLP_ENDPOINT || '/v1/traces'
const OTEL_ENABLED = import.meta.env.VITE_OTEL_ENABLED === 'true'

export async function initOtel() {
  if (!OTEL_ENABLED) {
    return
  }

  try {
    const [
      { WebTracerProvider },
      { BatchSpanProcessor },
      { OTLPTraceExporter },
      { Resource },
      semconv,
      { ZoneContextManager },
      { registerInstrumentations },
      { FetchInstrumentation },
      { XMLHttpRequestInstrumentation },
      { DocumentLoadInstrumentation },
      { UserInteractionInstrumentation },
      { W3CTraceContextPropagator },
      { propagation },
    ] = await Promise.all([
      import('@opentelemetry/sdk-trace-web'),
      import('@opentelemetry/sdk-trace-base'),
      import('@opentelemetry/exporter-trace-otlp-http'),
      import('@opentelemetry/resources'),
      import('@opentelemetry/semantic-conventions'),
      import('@opentelemetry/context-zone'),
      import('@opentelemetry/instrumentation'),
      import('@opentelemetry/instrumentation-fetch'),
      import('@opentelemetry/instrumentation-xml-http-request'),
      import('@opentelemetry/instrumentation-document-load'),
      import('@opentelemetry/instrumentation-user-interaction'),
      import('@opentelemetry/core'),
      import('@opentelemetry/api'),
    ])

    propagation.setGlobalPropagator(new W3CTraceContextPropagator())

    const resource = new Resource({
      [semconv.ATTR_SERVICE_NAME]: 'zentra-frontend',
      [semconv.ATTR_SERVICE_VERSION]: '1.0.0',
      'deployment.environment': import.meta.env.MODE || 'development',
    })

    const exporter = new OTLPTraceExporter({ url: OTEL_ENDPOINT })
    const provider = new WebTracerProvider({ resource })

    provider.addSpanProcessor(new BatchSpanProcessor(exporter, {
      maxQueueSize: 100,
      maxExportBatchSize: 10,
      scheduledDelayMillis: 5000,
    }))

    provider.register({ contextManager: new ZoneContextManager() })

    const apiOrigin = window.location.origin
    registerInstrumentations({
      instrumentations: [
        new FetchInstrumentation({
          propagateTraceHeaderCorsUrls: [
            new RegExp(apiOrigin.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')),
            /\/api\//, /\/v1\//,
          ],
        }),
        new XMLHttpRequestInstrumentation({
          propagateTraceHeaderCorsUrls: [
            new RegExp(apiOrigin.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')),
          ],
        }),
        new DocumentLoadInstrumentation(),
        new UserInteractionInstrumentation(),
      ],
    })

    console.info('[OTEL] Browser RUM initialized', { endpoint: OTEL_ENDPOINT, service: 'zentra-frontend' })
  } catch {
    // @opentelemetry packages not installed — skip silently
  }
}

// Auto-init on import
initOtel()
