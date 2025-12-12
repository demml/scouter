DO $$
DECLARE
    -- Configuration variables
    v_num_traces INTEGER := 1000;
    v_days_back INTEGER := 7;
    v_base_time TIMESTAMPTZ := NOW() - INTERVAL '7 days';

    -- Service definitions for realistic test data
    v_services TEXT[] := ARRAY[
        'user-service', 'auth-service', 'payment-service',
        'inventory-service', 'notification-service', 'analytics-service'
    ];

    v_operations TEXT[] := ARRAY[
        'GET /users', 'POST /users', 'PUT /users/{id}', 'DELETE /users/{id}',
        'POST /auth/login', 'POST /auth/refresh', 'POST /auth/logout',
        'POST /payments/charge', 'GET /payments/history', 'POST /payments/refund',
        'GET /inventory/items', 'PUT /inventory/stock', 'POST /inventory/reserve',
        'POST /notifications/send', 'GET /notifications/templates',
        'POST /analytics/events', 'GET /analytics/reports'
    ];

    v_span_kinds TEXT[] := ARRAY['server', 'client', 'producer', 'consumer', 'internal'];

    -- Loop variables
    i INTEGER;
    j INTEGER;
    k INTEGER;
    v_baggage_created_at TIMESTAMPTZ;
    v_baggage_sequence INTEGER := 0;
    v_tag_offset_ms INTEGER;

    -- Service tracking
    v_service_id INTEGER;
    v_service_map JSONB := '{}'::JSONB;

    -- Trace variables
    v_trace_id TEXT;
    v_root_span_id TEXT;
    v_current_time TIMESTAMPTZ;
    v_trace_duration BIGINT;
    v_service_name TEXT;
    v_operation TEXT;
    v_has_error BOOLEAN;
    v_span_count INTEGER;

    -- Span variables
    v_span_id TEXT;
    v_parent_span_id TEXT;
    v_span_start TIMESTAMPTZ;
    v_span_end TIMESTAMPTZ;
    v_span_duration BIGINT;
    v_span_status_code INTEGER;
    v_span_kind TEXT;

BEGIN
    RAISE NOTICE 'Starting test data population for % traces over % days', v_num_traces, v_days_back;

    -- 1. Create Service Entities
    RAISE NOTICE 'Creating service entities for % services', array_length(v_services, 1);

    FOR i IN 1..array_length(v_services, 1) LOOP
        v_service_name := v_services[i];

        INSERT INTO scouter.service_entities (service_name)
        VALUES (v_service_name)
        ON CONFLICT (service_name) DO UPDATE
            SET updated_at = NOW()
        RETURNING id INTO v_service_id;

        v_service_map := jsonb_set(
            v_service_map,
            ARRAY[v_service_name],
            to_jsonb(v_service_id)
        );

        RAISE NOTICE 'Created/found service_id % for service "%"', v_service_id, v_service_name;
    END LOOP;

    -- 2. Generate test traces (span-based architecture)
    FOR i IN 1..v_num_traces LOOP
        -- Generate unique trace ID
        v_trace_id := 'trace-' || LPAD(i::TEXT, 6, '0') || '-' || EXTRACT(EPOCH FROM NOW())::BIGINT;
        v_root_span_id := 'span-' || v_trace_id || '-root';

        -- Random timestamp within the last week
        v_current_time := v_base_time + (RANDOM() * INTERVAL '7 days');
        v_baggage_sequence := 0;

        -- Select random service and lookup service_id
        v_service_name := v_services[1 + (RANDOM() * (array_length(v_services, 1) - 1))::INTEGER];
        v_service_id := (v_service_map->v_service_name)::INTEGER;

        v_operation := v_operations[1 + (RANDOM() * (array_length(v_operations, 1) - 1))::INTEGER];

        -- Determine if trace has errors (20% chance)
        v_has_error := RANDOM() < 0.2;

        -- Generate realistic trace duration
        v_trace_duration := CASE
            WHEN RANDOM() < 0.9 THEN (50 + RANDOM() * 950)::BIGINT  -- 90% normal (50-1000ms)
            WHEN RANDOM() < 0.98 THEN (1000 + RANDOM() * 4000)::BIGINT  -- 8% slow (1-5s)
            ELSE (5000 + RANDOM() * 15000)::BIGINT  -- 2% very slow (5-20s)
        END;

        -- Generate realistic span count (2-15 spans per trace)
        v_span_count := 2 + (RANDOM() * 13)::INTEGER;

        -- Generate trace-level tags (attached to trace_id as entity)
        FOR k IN 1..(2 + (RANDOM() * 2)::INTEGER) LOOP
            INSERT INTO scouter.tags (
                created_at, entity_type, entity_id, key, value
            ) VALUES (
                v_current_time + (RANDOM() * INTERVAL '1 second'),
                'trace',
                v_trace_id,
                CASE
                    WHEN k = 1 THEN 'trace.tag.env'
                    WHEN k = 2 THEN 'trace.tag.region'
                    WHEN k = 3 THEN 'trace.tag.customer'
                    ELSE 'trace.tag.custom'
                END,
                CASE
                    WHEN k = 1 THEN 'prod-' || (ARRAY['us', 'eu', 'asia'])[1 + (RANDOM() * 2)::INTEGER]
                    WHEN k = 2 THEN (ARRAY['east', 'west'])[1 + (RANDOM() * 1)::INTEGER]
                    WHEN k = 3 THEN 'cust-' || (1 + RANDOM() * 10)::INTEGER
                    ELSE 'value-' || LPAD(k::TEXT, 2, '0')
                END
            );
        END LOOP;

        -- Generate spans for this trace
        v_span_start := v_current_time;
        v_parent_span_id := NULL;

        FOR j IN 1..v_span_count LOOP
            v_span_id := CASE
                WHEN j = 1 THEN v_root_span_id
                ELSE 'span-' || v_trace_id || '-' || j
            END;

            -- Set parent span
            IF j = 1 THEN
                v_parent_span_id := NULL;
            ELSIF j = 2 THEN
                v_parent_span_id := v_root_span_id;
            ELSE
                v_parent_span_id := CASE
                    WHEN RANDOM() < 0.7 THEN 'span-' || v_trace_id || '-' || (j-1)
                    ELSE v_root_span_id
                END;
            END IF;

            -- Generate span duration
            v_span_duration := CASE
                WHEN j = 1 THEN v_trace_duration
                ELSE (10 + RANDOM() * 490)::BIGINT
            END;

            v_span_start := v_current_time + ((j-1) * 50 || ' milliseconds')::INTERVAL;
            v_span_end := v_span_start + (v_span_duration || ' milliseconds')::INTERVAL;

            -- Determine span status
            v_span_status_code := CASE
                WHEN v_has_error AND j = v_span_count THEN 2
                WHEN v_has_error AND RANDOM() < 0.1 THEN 2
                ELSE 1
            END;

            v_span_kind := v_span_kinds[1 + (RANDOM() * (array_length(v_span_kinds, 1) - 1))::INTEGER];

            -- Insert span (core tracing primitive)
            INSERT INTO scouter.spans (
                span_id,
                trace_id,
                parent_span_id,
                service_id,
                service_name,
                scope,
                span_name,
                span_kind,
                start_time,
                end_time,
                duration_ms,
                status_code,
                status_message,
                attributes,
                events,
                links,
                resource_attributes,
                created_at
            ) VALUES (
                v_span_id,
                v_trace_id,
                v_parent_span_id,
                v_service_id,
                v_service_name,
                'distributed-tracing',
                CASE
                    WHEN j = 1 THEN v_operation
                    ELSE v_operation || '/step-' || j
                END,
                v_span_kind,
                v_span_start,
                v_span_end,
                v_span_duration,
                v_span_status_code,
                CASE WHEN v_span_status_code = 2 THEN 'Internal server error' ELSE NULL END,
                jsonb_build_array(
                    jsonb_build_object('key', 'service.name', 'value', v_service_name),
                    jsonb_build_object('key', 'span.kind', 'value', v_span_kind),
                    jsonb_build_object('key', 'component', 'value', CASE
                        WHEN v_span_kind = 'server' THEN 'http'
                        WHEN v_span_kind = 'client' THEN 'http-client'
                        WHEN v_span_kind = 'producer' THEN 'kafka'
                        WHEN v_span_kind = 'consumer' THEN 'kafka'
                        ELSE 'internal'
                    END),
                    jsonb_build_object('key', 'thread.id', 'value', (1000 + RANDOM() * 9000)::INTEGER::TEXT)
                ),
                CASE
                    WHEN v_span_status_code = 2 THEN jsonb_build_array(
                        jsonb_build_object(
                            'timestamp', EXTRACT(EPOCH FROM v_span_end)::BIGINT * 1000000000,
                            'name', 'exception',
                            'attributes', jsonb_build_array(
                                jsonb_build_object('key', 'exception.type', 'value', 'RuntimeError'),
                                jsonb_build_object('key', 'exception.message', 'value', 'Simulated error for testing'),
                                jsonb_build_object('key', 'exception.stacktrace', 'value', 'RuntimeError: Simulated error\n  at test.py:123')
                            )
                        )
                    )
                    ELSE jsonb_build_array(
                        jsonb_build_object(
                            'timestamp', EXTRACT(EPOCH FROM v_span_start + (v_span_duration/2 || ' milliseconds')::INTERVAL)::BIGINT * 1000000000,
                            'name', 'processing.started',
                            'attributes', jsonb_build_array(
                                jsonb_build_object('key', 'stage', 'value', 'middleware'),
                                jsonb_build_object('key', 'processing.items', 'value', (1 + RANDOM() * 100)::INTEGER::TEXT)
                            )
                        )
                    )
                END,
                '[]'::jsonb,
                -- Resource attributes (OpenTelemetry resource metadata)
                jsonb_build_object(
                    'service.name', v_service_name,
                    'service.version', '1.0.' || (RANDOM() * 10)::INTEGER,
                    'deployment.environment', (ARRAY['production', 'staging', 'development'])[1 + (RANDOM() * 2)::INTEGER],
                    'host.name', 'host-' || (1 + RANDOM() * 20)::INTEGER,
                    'process.runtime.name', 'python',
                    'process.runtime.version', '3.11.' || (RANDOM() * 5)::INTEGER
                ),
                v_current_time
            );

            -- Generate span tags (15% chance)
            v_tag_offset_ms := 0;
            IF RANDOM() < 0.15 THEN
                FOR k IN 1..(1 + (RANDOM() * 2)::INTEGER) LOOP
                    v_tag_offset_ms := v_tag_offset_ms + 1;
                    INSERT INTO scouter.tags (
                        created_at, entity_type, entity_id, key, value
                    ) VALUES (
                        v_span_start + (v_tag_offset_ms || ' milliseconds')::INTERVAL,
                        'trace',
                        v_trace_id,
                        CASE
                            WHEN k = 1 THEN 'span.tag.host'
                            WHEN k = 2 THEN 'span.tag.db.query'
                            ELSE 'span.tag.custom.' || k  -- Make additional keys unique
                        END,
                        CASE
                            WHEN k = 1 THEN 'host-' || (1 + RANDOM() * 5)::INTEGER
                            WHEN k = 2 THEN 'SELECT * FROM items WHERE id=' || (1000 + RANDOM() * 9000)::INTEGER
                            ELSE 'custom-value-' || k
                        END
                    )
                    ON CONFLICT (entity_type, entity_id, key)
                    DO UPDATE SET
                        value = EXCLUDED.value,
                        updated_at = NOW();
                END LOOP;
            END IF;

            -- Generate baggage (30% chance per trace, not per span)
            IF j = 1 AND RANDOM() < 0.3 THEN
                FOR k IN 1..(1 + (RANDOM() * 3)::INTEGER) LOOP
                    v_baggage_sequence := v_baggage_sequence + 1;
                    v_baggage_created_at := v_current_time + (v_baggage_sequence * INTERVAL '200 milliseconds');

                    INSERT INTO scouter.trace_baggage (
                        trace_id, scope, key, value, created_at
                    ) VALUES (
                        v_trace_id,
                        'distributed-tracing',
                        CASE k
                            WHEN 1 THEN 'user.tier'
                            WHEN 2 THEN 'request.priority'
                            WHEN 3 THEN 'experiment.variant'
                            ELSE 'custom.key.' || k
                        END,
                        CASE k
                            WHEN 1 THEN (ARRAY['premium', 'standard', 'basic'])[1 + (RANDOM() * 2)::INTEGER]
                            WHEN 2 THEN (ARRAY['high', 'medium', 'low'])[1 + (RANDOM() * 2)::INTEGER]
                            WHEN 3 THEN (ARRAY['control', 'variant-a', 'variant-b'])[1 + (RANDOM() * 2)::INTEGER]
                            ELSE 'value-' || k
                        END,
                        v_baggage_created_at
                    );
                END LOOP;
            END IF;
        END LOOP;

        -- Progress indicator
        IF i % 100 = 0 THEN
            RAISE NOTICE 'Generated % traces...', i;
        END IF;
    END LOOP;

    RAISE NOTICE 'Successfully generated % traces with spans and baggage', v_num_traces;

    -- Display summary statistics
    RAISE NOTICE 'Summary Statistics:';
    RAISE NOTICE '- Total service entities: %', (SELECT COUNT(*) FROM scouter.service_entities);
    RAISE NOTICE '- Total unique traces: %', (SELECT COUNT(DISTINCT trace_id) FROM scouter.spans);
    RAISE NOTICE '- Total spans: %', (SELECT COUNT(*) FROM scouter.spans);
    RAISE NOTICE '- Total baggage entries: %', (SELECT COUNT(*) FROM scouter.trace_baggage);
    RAISE NOTICE '- Total tag entries: %', (SELECT COUNT(*) FROM scouter.tags);
    RAISE NOTICE '- Traces with errors: %', (
        SELECT COUNT(DISTINCT trace_id)
        FROM scouter.spans
        WHERE status_code = 2
    );
    RAISE NOTICE '- Average spans per trace: %', (
        SELECT ROUND(AVG(span_count), 2)
        FROM (
            SELECT COUNT(*) as span_count
            FROM scouter.spans
            GROUP BY trace_id
        ) t
    );
    RAISE NOTICE '- Average trace duration: % ms', (
        SELECT ROUND(AVG(duration_ms), 2)
        FROM (
            SELECT EXTRACT(EPOCH FROM (MAX(end_time) - MIN(start_time))) * 1000 as duration_ms
            FROM scouter.spans
            WHERE parent_span_id IS NULL
            GROUP BY trace_id
        ) t
    );

END $$;