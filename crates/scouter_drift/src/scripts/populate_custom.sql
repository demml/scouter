-- Insert custom profile into scouter.drift_profile
INSERT INTO scouter.drift_profile (created_at, updated_at, name, space, version, profile, drift_type, active, schedule, next_run, previous_run)
VALUES
    (
        timezone('utc', now()),
        timezone('utc', now()),
        'model',
        'scouter',
        '0.1.0',
        '{
                  "config": {
                    "space": "scouter",
                    "name": "model",
                    "version": "0.1.0",
                    "sample_size": 25,
                    "sample": true,
                    "alert_config": {
                        "dispatch_config": {
                            "Console": {
                                "enabled": true
                            }
                        },
                        "schedule": "0 0 0 * * *",
                        "alert_conditions": {
                            "mse": {
                                "alert_threshold": "Above",
                                "alert_threshold_value": 3.0
                            },
                            "mae": {
                                "alert_threshold": "Above",
                                "alert_threshold_value": 2.0
                            }
                        }
                    },
                    "drift_type": "Custom"
                  },
                  "metrics": {
                    "mse": 12.0,
                    "mae": 13.0
                  },
                  "scouter_version": "0.3.3"
                }',
        'CUSTOM',
        true,
        '0 0 0 * * *',
        timezone('utc', now() - interval '1 days'),
        timezone('utc', now() - interval '2 days')
    );


-- populate observed_bin_count table with dummy data
DO $$
    DECLARE
        n INTEGER := 3; -- Number of records per metric
        created_at timestamp := timezone('utc', current_timestamp) + (random() * INTERVAL '1 minutes') + (random() * INTERVAL '1 second');
        metric_names TEXT[] := ARRAY['mae', 'mse'];
        metric_value FLOAT;
    BEGIN
        FOR i IN 1..n LOOP
                FOR j IN array_lower(metric_names, 1)..array_upper(metric_names, 1) LOOP
                        IF metric_names[j] = 'mae' THEN
                            metric_value := random() + 10.0;
                        ELSIF metric_names[j] = 'mse' THEN
                            metric_value := random() + 16.0;
                        END IF;

                        INSERT INTO scouter.custom_metrics (created_at, name, space, version, metric, value)
                        VALUES
                            (created_at + (random() * INTERVAL '1 second'), 'model', 'scouter', '0.1.0', metric_names[j], metric_value);
                    END LOOP;
            END LOOP;
    END $$;