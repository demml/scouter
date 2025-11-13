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
    v_status_codes TEXT[] := ARRAY['ok', 'error', 'timeout', 'cancelled'];
    
    -- Loop variables
    i INTEGER;
    j INTEGER;
    k INTEGER;
	v_baggage_created_at TIMESTAMPTZ;
	v_baggage_sequence INTEGER := 0;
	
    
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
    v_span_status TEXT;
    v_span_kind TEXT;
    
BEGIN
    RAISE NOTICE 'Starting test data population for % traces over % days', v_num_traces, v_days_back;
    
    -- Generate test traces
    FOR i IN 1..v_num_traces LOOP
        -- Generate unique trace ID
        v_trace_id := 'trace-' || LPAD(i::TEXT, 6, '0') || '-' || EXTRACT(EPOCH FROM NOW())::BIGINT;
        v_root_span_id := 'span-' || v_trace_id || '-root';
        
        -- Random timestamp within the last week
        v_current_time := v_base_time + (RANDOM() * INTERVAL '7 days');
		v_baggage_sequence := 0;
        
        -- Select random service and operation
        v_service_name := v_services[1 + (RANDOM() * (array_length(v_services, 1) - 1))::INTEGER];
        v_operation := v_operations[1 + (RANDOM() * (array_length(v_operations, 1) - 1))::INTEGER];
        
        -- Determine if trace has errors (20% chance)
        v_has_error := RANDOM() < 0.2;
        
        -- Generate realistic trace duration (50ms to 5000ms, with some outliers)
        v_trace_duration := CASE 
            WHEN RANDOM() < 0.9 THEN (50 + RANDOM() * 950)::BIGINT  -- 90% normal (50-1000ms)
            WHEN RANDOM() < 0.98 THEN (1000 + RANDOM() * 4000)::BIGINT  -- 8% slow (1-5s)
            ELSE (5000 + RANDOM() * 15000)::BIGINT  -- 2% very slow (5-20s)
        END;
        
        -- Generate realistic span count (2-15 spans per trace)
        v_span_count := 2 + (RANDOM() * 13)::INTEGER;
        
        -- Insert trace record
        INSERT INTO scouter.traces (
            trace_id, space, name, version, scope, trace_state,
            start_time, end_time, duration_ms, status, root_span_id,
            span_count, attributes, created_at
        ) VALUES (
            v_trace_id,
            'production',
            v_service_name,
            'v1.0.0',
            'distributed-tracing',
            'sampled=1',
            v_current_time,
            v_current_time + (v_trace_duration || ' milliseconds')::INTERVAL,
            v_trace_duration,
            CASE WHEN v_has_error THEN 'error' ELSE 'ok' END,
            v_root_span_id,
            v_span_count,
            -- CORRECTED: attributes JSONB must be an array of EAV objects
            jsonb_build_array(
                jsonb_build_object('key', 'service.name', 'value', v_service_name),
                jsonb_build_object('key', 'service.version', 'value', 'v1.0.0'),
                jsonb_build_object('key', 'deployment.environment', 'value', 'production'),
                jsonb_build_object('key', 'scouter.service.name', 'value', v_service_name),
                jsonb_build_object('key', 'http.method', 'value', SPLIT_PART(v_operation, ' ', 1)),
                jsonb_build_object('key', 'http.route', 'value', SPLIT_PART(v_operation, ' ', 2)),
                jsonb_build_object('key', 'user.id', 'value', 'user-' || (1000 + RANDOM() * 9000)::INTEGER::TEXT)
            ),
            v_current_time
        );

        -- Generate 2-4 tags for the Trace entity
        FOR k IN 1..(2 + (RANDOM() * 2)::INTEGER) LOOP
            INSERT INTO scouter.tags (
                created_at, entity_type, entity_id, key, value
            ) VALUES (
                v_current_time + (RANDOM() * INTERVAL '1 second'), -- slight jitter to created_at
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
            
            -- Set parent span (root span has no parent, others randomly choose parent)
            IF j = 1 THEN
                v_parent_span_id := NULL;
            ELSIF j = 2 THEN
                v_parent_span_id := v_root_span_id;
            ELSE
                -- 70% chance to use immediate parent, 30% chance to use root
                v_parent_span_id := CASE 
                    WHEN RANDOM() < 0.7 THEN 'span-' || v_trace_id || '-' || (j-1)
                    ELSE v_root_span_id
                END;
            END IF;
            
            -- Generate span duration (10-500ms for most spans)
            v_span_duration := CASE
                WHEN j = 1 THEN v_trace_duration  -- Root span duration = trace duration
                ELSE (10 + RANDOM() * 490)::BIGINT
            END;
            
            -- Adjust span times to fit within trace
            v_span_start := v_current_time + ((j-1) * 50 || ' milliseconds')::INTERVAL;
            v_span_end := v_span_start + (v_span_duration || ' milliseconds')::INTERVAL;
            
            -- Determine span status
            v_span_status := CASE
                WHEN v_has_error AND j = v_span_count THEN 'error'  -- Last span gets the error
                WHEN v_has_error AND RANDOM() < 0.1 THEN 'error'    -- 10% chance other spans error
                ELSE 'ok'
            END;
            
            -- Random span kind
            v_span_kind := v_span_kinds[1 + (RANDOM() * (array_length(v_span_kinds, 1) - 1))::INTEGER];
            
            -- Insert span
            INSERT INTO scouter.spans (
                span_id, trace_id, parent_span_id, space, name, version, scope,
                span_name, span_kind, start_time, end_time, duration_ms,
                status_code, status_message, attributes, events, links, created_at
            ) VALUES (
                v_span_id,
                v_trace_id,
                v_parent_span_id,
                'production',
                v_service_name,
                'v1.0.0',
                'distributed-tracing',
                CASE 
                    WHEN j = 1 THEN v_operation
                    ELSE v_operation || '/step-' || j
                END,
                v_span_kind,
                v_span_start,
                v_span_end,
                v_span_duration,
                v_span_status,
                CASE WHEN v_span_status = 'error' THEN 'Internal server error' ELSE NULL END,
                -- CORRECTED: attributes JSONB must be an array of EAV objects
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
                    jsonb_build_object('key', 'scouter.model.name', 'value', v_service_name || '-model'),
                    jsonb_build_object('key', 'scouter.feature.count', 'value', (5 + RANDOM() * 20)::INTEGER::TEXT),
                    jsonb_build_object('key', 'thread.id', 'value', (1000 + RANDOM() * 9000)::INTEGER::TEXT)
                ),
                CASE 
                    WHEN v_span_status = 'error' THEN jsonb_build_array(
                        jsonb_build_object(
                            'timestamp', EXTRACT(EPOCH FROM v_span_end)::BIGINT * 1000000000,
                            'name', 'exception',
                            -- CORRECTED: nested attributes must also be an array of EAV objects
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
                            -- CORRECTED: nested attributes must also be an array of EAV objects
                            'attributes', jsonb_build_array(
                                jsonb_build_object('key', 'stage', 'value', 'middleware'),
                                jsonb_build_object('key', 'processing.items', 'value', (1 + RANDOM() * 100)::INTEGER::TEXT)
                            )
                        )
                    )
                END,
                '[]'::jsonb,  -- Empty links for simplicity (structure would be similar to events)
                v_current_time
            );

            -- Generate 1-3 tags for the Span entity (15% chance for tags)
            IF RANDOM() < 0.15 THEN
                FOR k IN 1..(1 + (RANDOM() * 2)::INTEGER) LOOP
                    INSERT INTO scouter.tags (
                        created_at, entity_type, entity_id, key, value
                    ) VALUES (
                        v_span_start + (RANDOM() * v_span_duration * 0.1 || ' milliseconds')::INTERVAL,
                        'span',
                        v_span_id,
                        CASE 
                            WHEN k = 1 THEN 'span.tag.host'
                            ELSE 'span.tag.db.query'
                        END,
                        CASE 
                            WHEN k = 1 THEN 'host-' || (1 + RANDOM() * 5)::INTEGER
                            ELSE 'SELECT * FROM items WHERE id=' || (1000 + RANDOM() * 9000)::INTEGER
                        END
                    );
                END LOOP;
            END IF;

            -- Generate baggage for some spans (30% chance)
            IF RANDOM() < 0.3 THEN
                FOR k IN 1..(1 + (RANDOM() * 3)::INTEGER) LOOP
                    
                    -- Increment sequence and add 200ms per baggage entry
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
    
    -- Refresh materialized view to include new data
    REFRESH MATERIALIZED VIEW scouter.trace_summary;
    
    RAISE NOTICE 'Refreshed trace_summary materialized view';
    
    -- Display summary statistics
    RAISE NOTICE 'Summary Statistics:';
    RAISE NOTICE '- Total traces: %', (SELECT COUNT(*) FROM scouter.traces);
    RAISE NOTICE '- Total spans: %', (SELECT COUNT(*) FROM scouter.spans);
    RAISE NOTICE '- Total baggage entries: %', (SELECT COUNT(*) FROM scouter.trace_baggage);
    RAISE NOTICE '- Total tag entries: %', (SELECT COUNT(*) FROM scouter.tags); -- ADDED
    RAISE NOTICE '- Traces with errors: %', (SELECT COUNT(*) FROM scouter.traces WHERE status != 'ok');
    RAISE NOTICE '- Average spans per trace: %', (SELECT ROUND(AVG(span_count), 2) FROM scouter.traces);
    RAISE NOTICE '- Average trace duration: % ms', (SELECT ROUND(AVG(duration_ms), 2) FROM scouter.traces);
    
END $$;