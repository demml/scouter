-- Insert psi profile into drift_profile
INSERT INTO scouter.drift_entities (uid, id, space, name, version, drift_type)
VALUES ('019ae1b4-3003-77c1-8eed-2ec005e85963', 1, 'scouter', 'model', '0.1.0', 'Psi')
ON CONFLICT (space, name, version, drift_type) DO NOTHING;


INSERT INTO scouter.drift_profile (created_at, updated_at, name, space, version, profile, drift_type, active, schedule,
                                   next_run, previous_run, entity_id)
VALUES (timezone('utc', NOW()),
        timezone('utc', NOW()),
        'model',
        'scouter',
        '0.1.0',
        '{
           "features": {
              "feature_1": {
                 "id": "feature_1",
                 "bins": [
                    {
                       "id": 1,
                       "lower_limit": null,
                       "upper_limit": -19.44250690708471,
                       "proportion": 0.1
                    },
                    {
                       "id": 2,
                       "lower_limit": -19.44250690708471,
                       "upper_limit": -11.814483949783837,
                       "proportion": 0.1
                    },
                    {
                       "id": 3,
                       "lower_limit": -11.814483949783837,
                       "upper_limit": -5.985070107363507,
                       "proportion": 0.1
                    },
                    {
                       "id": 4,
                       "lower_limit": -5.985070107363507,
                       "upper_limit": -1.1426337797283574,
                       "proportion": 0.1
                    },
                    {
                       "id": 5,
                       "lower_limit": -1.1426337797283574,
                       "upper_limit": 2.7283634173268565,
                       "proportion": 0.1
                    },
                    {
                       "id": 6,
                       "lower_limit": 2.7283634173268565,
                       "upper_limit": 7.123546298901827,
                       "proportion": 0.1
                    },
                    {
                       "id": 7,
                       "lower_limit": 7.123546298901827,
                       "upper_limit": 11.579204339860262,
                       "proportion": 0.1
                    },
                    {
                       "id": 8,
                       "lower_limit": 11.579204339860262,
                       "upper_limit": 17.608411332277463,
                       "proportion": 0.1
                    },
                    {
                       "id": 9,
                       "lower_limit": 17.608411332277463,
                       "upper_limit": 26.110007426019052,
                       "proportion": 0.1
                    },
                    {
                       "id": 10,
                       "lower_limit": 26.110007426019052,
                       "upper_limit": null,
                       "proportion": 0.1
                    }
                 ],
                 "timestamp": "2024-11-12T20:29:51.195989Z",
                 "bin_type": "Numeric"
              },
              "target": {
                 "id": "target",
                 "bins": [
                    {
                       "id": 1,
                       "lower_limit": null,
                       "upper_limit": 1.0,
                       "proportion": 0.119
                    },
                    {
                       "id": 2,
                       "lower_limit": 1.0,
                       "upper_limit": 2.0,
                       "proportion": 0.117
                    },
                    {
                       "id": 3,
                       "lower_limit": 2.0,
                       "upper_limit": 3.0,
                       "proportion": 0.114
                    },
                    {
                       "id": 4,
                       "lower_limit": 3.0,
                       "upper_limit": 4.0,
                       "proportion": 0.103
                    },
                    {
                       "id": 5,
                       "lower_limit": 4.0,
                       "upper_limit": 5.0,
                       "proportion": 0.129
                    },
                    {
                       "id": 6,
                       "lower_limit": 5.0,
                       "upper_limit": 6.0,
                       "proportion": 0.099
                    },
                    {
                       "id": 7,
                       "lower_limit": 6.0,
                       "upper_limit": 7.0,
                       "proportion": 0.103
                    },
                    {
                       "id": 8,
                       "lower_limit": 7.0,
                       "upper_limit": 8.0,
                       "proportion": 0.103
                    },
                    {
                       "id": 9,
                       "lower_limit": 8.0,
                       "upper_limit": 9.0,
                       "proportion": 0.113
                    },
                    {
                       "id": 10,
                       "lower_limit": 9.0,
                       "upper_limit": null,
                       "proportion": 0.0
                    }
                 ],
                 "timestamp": "2024-11-12T20:29:51.195710Z",
                 "bin_type": "Numeric"
              },
              "feature_2": {
                 "id": "feature_2",
                 "bins": [
                    {
                       "id": 1,
                       "lower_limit": null,
                       "upper_limit": -17.72822708903977,
                       "proportion": 0.1
                    },
                    {
                       "id": 2,
                       "lower_limit": -17.72822708903977,
                       "upper_limit": -10.878016912630766,
                       "proportion": 0.1
                    },
                    {
                       "id": 3,
                       "lower_limit": -10.878016912630766,
                       "upper_limit": -5.113617477362791,
                       "proportion": 0.1
                    },
                    {
                       "id": 4,
                       "lower_limit": -5.113617477362791,
                       "upper_limit": -0.8665699954397752,
                       "proportion": 0.1
                    },
                    {
                       "id": 5,
                       "lower_limit": -0.8665699954397752,
                       "upper_limit": 3.4170113920330927,
                       "proportion": 0.1
                    },
                    {
                       "id": 6,
                       "lower_limit": 3.4170113920330927,
                       "upper_limit": 8.897572066092266,
                       "proportion": 0.1
                    },
                    {
                       "id": 7,
                       "lower_limit": 8.897572066092266,
                       "upper_limit": 13.865560881490225,
                       "proportion": 0.1
                    },
                    {
                       "id": 8,
                       "lower_limit": 13.865560881490225,
                       "upper_limit": 19.125534988440656,
                       "proportion": 0.1
                    },
                    {
                       "id": 9,
                       "lower_limit": 19.125534988440656,
                       "upper_limit": 27.39857353135026,
                       "proportion": 0.1
                    },
                    {
                       "id": 10,
                       "lower_limit": 27.39857353135026,
                       "upper_limit": null,
                       "proportion": 0.1
                    }
                 ],
                 "timestamp": "2024-11-12T20:29:51.195986Z",
                 "bin_type": "Numeric"
              },
              "feature_3": {
                 "id": "feature_3",
                 "bins": [
                    {
                       "id": 1,
                       "lower_limit": null,
                       "upper_limit": -19.935132943683904,
                       "proportion": 0.1
                    },
                    {
                       "id": 2,
                       "lower_limit": -19.935132943683904,
                       "upper_limit": -12.791952128625383,
                       "proportion": 0.1
                    },
                    {
                       "id": 3,
                       "lower_limit": -12.791952128625383,
                       "upper_limit": -7.38670301599155,
                       "proportion": 0.1
                    },
                    {
                       "id": 4,
                       "lower_limit": -7.38670301599155,
                       "upper_limit": -3.0888240237654623,
                       "proportion": 0.1
                    },
                    {
                       "id": 5,
                       "lower_limit": -3.0888240237654623,
                       "upper_limit": 1.9995930102913435,
                       "proportion": 0.1
                    },
                    {
                       "id": 6,
                       "lower_limit": 1.9995930102913435,
                       "upper_limit": 6.830513995025102,
                       "proportion": 0.1
                    },
                    {
                       "id": 7,
                       "lower_limit": 6.830513995025102,
                       "upper_limit": 12.513987736919315,
                       "proportion": 0.1
                    },
                    {
                       "id": 8,
                       "lower_limit": 12.513987736919315,
                       "upper_limit": 17.68595891293836,
                       "proportion": 0.1
                    },
                    {
                       "id": 9,
                       "lower_limit": 17.68595891293836,
                       "upper_limit": 24.763507245168494,
                       "proportion": 0.1
                    },
                    {
                       "id": 10,
                       "lower_limit": 24.763507245168494,
                       "upper_limit": null,
                       "proportion": 0.1
                    }
                 ],
                 "timestamp": "2024-11-12T20:29:51.195978Z",
                 "bin_type": "Numeric"
              }
           },
           "config": {
              "space": "scouter",
              "name": "model",
              "version": "0.1.0",
              "uid": "019ae1b4-3003-77c1-8eed-2ec005e85963",
              "binning_strategy": {
                 "QuantileBinning": {
                    "num_bins": 10
                 }
              },
              "feature_map": {
                 "features": {}
              },
              "alert_config": {
                 "dispatch_config": {
                    "Console": {
                       "enabled": true
                    }
                 },
                 "schedule": "0 0 0 * * *",
                 "features_to_monitor": [
                    "feature_1",
                    "feature_2",
                    "feature_3"
                 ],
                 "threshold": {
                    "ChiSquare": {
                       "alpha": 0.05
                    }
                 }
              },
              "drift_type": "Psi"
           },
           "scouter_version": "0.3.2"
        }',
        'PSI',
        TRUE,
        '0 0 0 * * *',
        timezone('utc', NOW() - INTERVAL '1 days'),
        timezone('utc', NOW() - INTERVAL '2 days'),
        1
        );


-- populate psi_drift table with dummy data
DO
$$
    DECLARE
        created_at_1 timestamp := timezone('utc', CURRENT_TIMESTAMP - INTERVAL '1 days') +
                                  (RANDOM() * INTERVAL '1 minutes') + (RANDOM() * INTERVAL '1 second');
        entity_id    integer      := 1;
        feature      text;
        bin_id       integer;
        bin_count_calc INTEGER;
        bin_count    INTEGER   := {{bin_count}};
        skew_feature text      := '{{skew_feature}}';
        skew_factor  INTEGER   := {{skew_factor}};
        apply_skew   BOOLEAN   := {{apply_skew}};
    BEGIN
        feature := 'feature_1';
        FOR i IN 1..10
            LOOP
                bin_id := i;
                bin_count_calc := CASE
                                      WHEN apply_skew AND feature = skew_feature AND MOD(i, 2) = 0
                                          THEN bin_count * skew_factor  -- Use base value
                                      ELSE bin_count
                    END;
                INSERT INTO scouter.psi_drift (created_at, entity_id, feature, bin_id, bin_count)
                VALUES (created_at_1, entity_id, feature, bin_id, bin_count_calc);
            END LOOP;

        feature := 'feature_2';
        FOR i IN 1..10
            LOOP
                bin_id := i;
                bin_count_calc := CASE
                                      WHEN apply_skew AND feature = skew_feature AND MOD(i, 2) = 0
                                          THEN bin_count * skew_factor  -- Use base value
                                      ELSE bin_count
                    END;
                INSERT INTO scouter.psi_drift (created_at, entity_id, feature, bin_id, bin_count)
                VALUES (created_at_1, entity_id, feature, bin_id, bin_count_calc);
            END LOOP;

        feature := 'feature_3';
        FOR i IN 1..10
            LOOP
                bin_id := i;
                bin_count_calc := CASE
                                      WHEN apply_skew AND feature = skew_feature AND MOD(i, 2) = 0
                                          THEN bin_count * skew_factor  -- Use base value
                                      ELSE bin_count
                    END;
                INSERT INTO scouter.psi_drift (created_at, entity_id, feature, bin_id, bin_count)
                VALUES (created_at_1, entity_id, feature, bin_id, bin_count_calc);
                -- ...
            END LOOP;
    END
$$;


DO
$$
    DECLARE
        entity_id    integer      := 1;
        feature      text;
        bin_id       integer;
        bin_count_calc INTEGER;
        bin_count    INTEGER   := {{bin_count}};
        skew_feature text      := '{{skew_feature}}';
        skew_factor  INTEGER   := {{skew_factor}};
        apply_skew   BOOLEAN   := {{apply_skew}};
        created_at_2 timestamp := timezone('utc', CURRENT_TIMESTAMP - INTERVAL '1 days') +
                                  (RANDOM() * INTERVAL '1 minutes') + (RANDOM() * INTERVAL '5 second');
    BEGIN
        feature := 'feature_1';
        FOR i IN 1..10
            LOOP
                bin_id := i;
                bin_count_calc := CASE
                                 WHEN apply_skew AND feature = skew_feature AND MOD(i, 2) = 0
                                     THEN bin_count * skew_factor
                                 ELSE bin_count
                    END;
                INSERT INTO scouter.psi_drift (created_at, entity_id, feature, bin_id, bin_count)
                VALUES (created_at_2, entity_id, feature, bin_id, bin_count_calc);
                created_at_2 := created_at_2 + (RANDOM() * INTERVAL '1 second');
            END LOOP;

        feature := 'feature_2';
        FOR i IN 1..10
            LOOP
                bin_id := i;
                bin_count_calc := CASE
                                      WHEN apply_skew AND feature = skew_feature AND MOD(i, 2) = 0
                                          THEN bin_count * skew_factor
                                      ELSE bin_count
                    END;
                INSERT INTO scouter.psi_drift (created_at, entity_id, feature, bin_id, bin_count)
                VALUES (created_at_2, entity_id, feature, bin_id, bin_count_calc);
                created_at_2 := created_at_2 + (RANDOM() * INTERVAL '1 second');
            END LOOP;

        feature := 'feature_3';
        FOR i IN 1..10
            LOOP
                bin_id := i;
                bin_count_calc := CASE
                                      WHEN apply_skew AND feature = skew_feature AND MOD(i, 2) = 0
                                          THEN bin_count * skew_factor
                                      ELSE bin_count
                    END;
                INSERT INTO scouter.psi_drift (created_at, entity_id, feature, bin_id, bin_count)
                VALUES (created_at_2, entity_id, feature, bin_id, bin_count_calc);
                created_at_2 := created_at_2 + (RANDOM() * INTERVAL '1 second');
            END LOOP;
    END
$$;

DO
$$
    DECLARE
        entity_id    integer      := 1;
        feature      text;
        bin_id       integer;
        bin_count_calc INTEGER;
        bin_count    INTEGER   := {{bin_count}};
        skew_feature text      := '{{skew_feature}}';
        skew_factor  INTEGER   := {{skew_factor}};
        apply_skew   BOOLEAN   := {{apply_skew}};
        created_at_3 timestamp := timezone('utc', CURRENT_TIMESTAMP - INTERVAL '1 days') +
                                  (RANDOM() * INTERVAL '1 minutes') + (RANDOM() * INTERVAL '3 second');
    BEGIN
        feature := 'feature_1';
        FOR i IN 1..10
            LOOP
                bin_id := i;
                bin_count_calc := CASE
                                 WHEN apply_skew AND feature = skew_feature AND MOD(i, 2) = 0
                                     THEN bin_count * skew_factor
                                 ELSE bin_count
                    END;
                INSERT INTO scouter.psi_drift (created_at, entity_id, feature, bin_id, bin_count)
                VALUES (created_at_3, entity_id, feature, bin_id, bin_count_calc);
                created_at_3 := created_at_3 + (RANDOM() * INTERVAL '1 second');
            END LOOP;

        feature := 'feature_2';
        FOR i IN 1..10
            LOOP
                bin_id := i;
                bin_count_calc := CASE
                                      WHEN apply_skew AND feature = skew_feature AND MOD(i, 2) = 0
                                          THEN bin_count * skew_factor
                                      ELSE bin_count
                    END;
                INSERT INTO scouter.psi_drift (created_at, entity_id, feature, bin_id, bin_count)
                VALUES (created_at_3, entity_id, feature, bin_id, bin_count_calc);
                created_at_3 := created_at_3 + (RANDOM() * INTERVAL '1 second');
            END LOOP;

        feature := 'feature_3';
        FOR i IN 1..10
            LOOP
                bin_id := i;
                bin_count_calc := CASE
                                      WHEN apply_skew AND feature = skew_feature AND MOD(i, 2) = 0
                                          THEN bin_count * skew_factor
                                      ELSE bin_count
                    END;
                INSERT INTO scouter.psi_drift (created_at, entity_id, feature, bin_id, bin_count)
                VALUES (created_at_3, entity_id, feature, bin_id, bin_count_calc);
                created_at_3 := created_at_3 + (RANDOM() * INTERVAL '1 second');
            END LOOP;
    END
$$;