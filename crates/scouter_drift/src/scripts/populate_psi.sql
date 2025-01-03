-- Insert psi profile into drift_profile
INSERT INTO drift_profile (created_at, updated_at, name, repository, version, profile, drift_type, active, schedule, next_run, previous_run)
VALUES
    (
        timezone('utc', now()),
        timezone('utc', now()),
        'model',
        'scouter',
        '0.1.0',
        '{
                  "features": {
                    "feature_1": {
                      "id": "feature_1",
                      "bins": [
                        {
                          "id": "decile_1",
                          "lower_limit": null,
                          "upper_limit": -19.44250690708471,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_2",
                          "lower_limit": -19.44250690708471,
                          "upper_limit": -11.814483949783837,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_3",
                          "lower_limit": -11.814483949783837,
                          "upper_limit": -5.985070107363507,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_4",
                          "lower_limit": -5.985070107363507,
                          "upper_limit": -1.1426337797283574,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_5",
                          "lower_limit": -1.1426337797283574,
                          "upper_limit": 2.7283634173268565,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_6",
                          "lower_limit": 2.7283634173268565,
                          "upper_limit": 7.123546298901827,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_7",
                          "lower_limit": 7.123546298901827,
                          "upper_limit": 11.579204339860262,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_8",
                          "lower_limit": 11.579204339860262,
                          "upper_limit": 17.608411332277463,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_9",
                          "lower_limit": 17.608411332277463,
                          "upper_limit": 26.110007426019052,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_10",
                          "lower_limit": 26.110007426019052,
                          "upper_limit": null,
                          "proportion": 0.1
                        }
                      ],
                      "timestamp": "2024-11-12T20:29:51.195989"
                    },
                    "target": {
                      "id": "target",
                      "bins": [
                        {
                          "id": "decile_1",
                          "lower_limit": null,
                          "upper_limit": 1.0,
                          "proportion": 0.119
                        },
                        {
                          "id": "decile_2",
                          "lower_limit": 1.0,
                          "upper_limit": 2.0,
                          "proportion": 0.117
                        },
                        {
                          "id": "decile_3",
                          "lower_limit": 2.0,
                          "upper_limit": 3.0,
                          "proportion": 0.114
                        },
                        {
                          "id": "decile_4",
                          "lower_limit": 3.0,
                          "upper_limit": 4.0,
                          "proportion": 0.103
                        },
                        {
                          "id": "decile_5",
                          "lower_limit": 4.0,
                          "upper_limit": 5.0,
                          "proportion": 0.129
                        },
                        {
                          "id": "decile_6",
                          "lower_limit": 5.0,
                          "upper_limit": 6.0,
                          "proportion": 0.099
                        },
                        {
                          "id": "decile_7",
                          "lower_limit": 6.0,
                          "upper_limit": 7.0,
                          "proportion": 0.103
                        },
                        {
                          "id": "decile_8",
                          "lower_limit": 7.0,
                          "upper_limit": 8.0,
                          "proportion": 0.103
                        },
                        {
                          "id": "decile_9",
                          "lower_limit": 8.0,
                          "upper_limit": 9.0,
                          "proportion": 0.113
                        },
                        {
                          "id": "decile_10",
                          "lower_limit": 9.0,
                          "upper_limit": null,
                          "proportion": 0.0
                        }
                      ],
                      "timestamp": "2024-11-12T20:29:51.195710"
                    },
                    "feature_2": {
                      "id": "feature_2",
                      "bins": [
                        {
                          "id": "decile_1",
                          "lower_limit": null,
                          "upper_limit": -17.72822708903977,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_2",
                          "lower_limit": -17.72822708903977,
                          "upper_limit": -10.878016912630766,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_3",
                          "lower_limit": -10.878016912630766,
                          "upper_limit": -5.113617477362791,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_4",
                          "lower_limit": -5.113617477362791,
                          "upper_limit": -0.8665699954397752,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_5",
                          "lower_limit": -0.8665699954397752,
                          "upper_limit": 3.4170113920330927,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_6",
                          "lower_limit": 3.4170113920330927,
                          "upper_limit": 8.897572066092266,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_7",
                          "lower_limit": 8.897572066092266,
                          "upper_limit": 13.865560881490225,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_8",
                          "lower_limit": 13.865560881490225,
                          "upper_limit": 19.125534988440656,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_9",
                          "lower_limit": 19.125534988440656,
                          "upper_limit": 27.39857353135026,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_10",
                          "lower_limit": 27.39857353135026,
                          "upper_limit": null,
                          "proportion": 0.1
                        }
                      ],
                      "timestamp": "2024-11-12T20:29:51.195986"
                    },
                    "feature_3": {
                      "id": "feature_3",
                      "bins": [
                        {
                          "id": "decile_1",
                          "lower_limit": null,
                          "upper_limit": -19.935132943683904,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_2",
                          "lower_limit": -19.935132943683904,
                          "upper_limit": -12.791952128625383,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_3",
                          "lower_limit": -12.791952128625383,
                          "upper_limit": -7.38670301599155,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_4",
                          "lower_limit": -7.38670301599155,
                          "upper_limit": -3.0888240237654623,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_5",
                          "lower_limit": -3.0888240237654623,
                          "upper_limit": 1.9995930102913435,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_6",
                          "lower_limit": 1.9995930102913435,
                          "upper_limit": 6.830513995025102,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_7",
                          "lower_limit": 6.830513995025102,
                          "upper_limit": 12.513987736919315,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_8",
                          "lower_limit": 12.513987736919315,
                          "upper_limit": 17.68595891293836,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_9",
                          "lower_limit": 17.68595891293836,
                          "upper_limit": 24.763507245168494,
                          "proportion": 0.1
                        },
                        {
                          "id": "decile_10",
                          "lower_limit": 24.763507245168494,
                          "upper_limit": null,
                          "proportion": 0.1
                        }
                      ],
                      "timestamp": "2024-11-12T20:29:51.195978"
                    }
                  },
                  "config": {
                    "repository": "scouter",
                    "name": "model",
                    "version": "0.1.0",
                    "feature_map": {
                      "features": {}
                    },
                    "alert_config": {
                      "dispatch_type": "Console",
                      "schedule": "0 0 0 * * *",
                      "features_to_monitor": [
                        "feature_1",
                        "feature_2",
                        "feature_3"
                      ],
                      "dispatch_kwargs": {},
                      "psi_threshold": 0.03
                    },
                    "targets": [
                      "target"
                    ],
                    "drift_type": "Psi"
                  },
                  "scouter_version": "0.3.2"
                }',
        'PSI',
        true,
        '0 0 0 * * *',
        timezone('utc', now() - interval '1 days'),
        timezone('utc', now() - interval '2 days')
    );


-- populate observed_bin_count table with dummy data
DO $$
    DECLARE
        created_at_1 timestamp := timezone('utc', current_timestamp - interval '1 days') + (random() * INTERVAL '1 minutes') + (random() * INTERVAL '1 second');
        name varchar(256) := 'model';
        repository varchar(256) := 'scouter';
        version varchar(256) := '0.1.0';
        feature varchar(256);
        bin_id varchar(256);
        bin_count integer;
    BEGIN
        feature := 'feature_1';
        FOR i IN 1..10 LOOP
                bin_id := 'decile_' || i;
                bin_count := (random() * 1000)::integer; -- random integer between 0 and 1000
                INSERT INTO observed_bin_count (created_at, name, repository, version, feature, bin_id, bin_count)
                VALUES (created_at_1, name, repository, version, feature, bin_id, bin_count);
                created_at_1 := created_at_1 + (random() * INTERVAL '1 second'); -- Adjust time slightly for each row
            END LOOP;

        feature := 'feature_2';
        FOR i IN 1..10 LOOP
                bin_id := 'decile_' || i;
                bin_count := (random() * 1000)::integer;
                INSERT INTO observed_bin_count (created_at, name, repository, version, feature, bin_id, bin_count)
                VALUES (created_at_1, name, repository, version, feature, bin_id, bin_count);
                created_at_1 := created_at_1 + (random() * INTERVAL '1 second');
            END LOOP;

        feature := 'feature_3';
        FOR i IN 1..10 LOOP
                bin_id := 'decile_' || i;
                bin_count := (random() * 1000)::integer;
                INSERT INTO observed_bin_count (created_at, name, repository, version, feature, bin_id, bin_count)
                VALUES (created_at_1, name, repository, version, feature, bin_id, bin_count);
                created_at_1 := created_at_1 + (random() * INTERVAL '1 second');
            END LOOP;
    END $$;


DO $$
    DECLARE
        name varchar(256) := 'model';
        repository varchar(256) := 'scouter';
        version varchar(256) := '0.1.0';
        feature varchar(256);
        bin_id varchar(256);
        bin_count integer;
        created_at_2 timestamp := timezone('utc', current_timestamp - interval '1 days') + (random() * INTERVAL '1 minutes') + (random() * INTERVAL '5 second');
    BEGIN
        feature := 'feature_1';
        FOR i IN 1..10 LOOP
                bin_id := 'decile_' || i;
                bin_count := (random() * 1000)::integer;
                INSERT INTO observed_bin_count (created_at, name, repository, version, feature, bin_id, bin_count)
                VALUES (created_at_2, name, repository, version, feature, bin_id, bin_count);
                created_at_2 := created_at_2 + (random() * INTERVAL '1 second');
            END LOOP;

        feature := 'feature_2';
        FOR i IN 1..10 LOOP
                bin_id := 'decile_' || i;
                bin_count := (random() * 1000)::integer;
                INSERT INTO observed_bin_count (created_at, name, repository, version, feature, bin_id, bin_count)
                VALUES (created_at_2, name, repository, version, feature, bin_id, bin_count);
                created_at_2 := created_at_2 + (random() * INTERVAL '1 second');
            END LOOP;

        feature := 'feature_3';
        FOR i IN 1..10 LOOP
                bin_id := 'decile_' || i;
                bin_count := (random() * 1000)::integer;
                INSERT INTO observed_bin_count (created_at, name, repository, version, feature, bin_id, bin_count)
                VALUES (created_at_2, name, repository, version, feature, bin_id, bin_count);
                created_at_2 := created_at_2 + (random() * INTERVAL '1 second');
            END LOOP;
    END $$;

DO $$
    DECLARE
        name varchar(256) := 'model';
        repository varchar(256) := 'scouter';
        version varchar(256) := '0.1.0';
        feature varchar(256);
        bin_id varchar(256);
        bin_count integer;
        created_at_3 timestamp := timezone('utc', current_timestamp - interval '1 days') + (random() * INTERVAL '1 minutes') + (random() * INTERVAL '3 second');
    BEGIN
        feature := 'feature_1';
        FOR i IN 1..10 LOOP
                bin_id := 'decile_' || i;
                bin_count := (random() * 1000)::integer;
                INSERT INTO observed_bin_count (created_at, name, repository, version, feature, bin_id, bin_count)
                VALUES (created_at_3, name, repository, version, feature, bin_id, bin_count);
                created_at_3 := created_at_3 + (random() * INTERVAL '1 second');
            END LOOP;

        feature := 'feature_2';
        FOR i IN 1..10 LOOP
                bin_id := 'decile_' || i;
                bin_count := (random() * 1000)::integer;
                INSERT INTO observed_bin_count (created_at, name, repository, version, feature, bin_id, bin_count)
                VALUES (created_at_3, name, repository, version, feature, bin_id, bin_count);
                created_at_3 := created_at_3 + (random() * INTERVAL '1 second');
            END LOOP;

        feature := 'feature_3';
        FOR i IN 1..10 LOOP
                bin_id := 'decile_' || i;
                bin_count := (random() * 1000)::integer;
                INSERT INTO observed_bin_count (created_at, name, repository, version, feature, bin_id, bin_count)
                VALUES (created_at_3, name, repository, version, feature, bin_id, bin_count);
                created_at_3 := created_at_3 + (random() * INTERVAL '1 second');
            END LOOP;
    END $$;