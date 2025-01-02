-- Insert initial data into drift_profile
INSERT INTO drift_profile (created_at, updated_at, name, repository, version, profile, drift_type, active, schedule, next_run, previous_run)
VALUES
  (
    timezone('utc', now()),
    timezone('utc', now()),
    'test_app',
    'statworld',
    '0.1.0',
    '{
      "features": {
        "col_2": {
              "id": "col_2",
              "center": -4.090610111507429,
              "one_ucl": -2.146102177058493,
              "one_lcl": -6.035118045956365,
              "two_ucl": -0.20159424260955694,
              "two_lcl": -7.9796259804053005,
              "three_ucl": 1.7429136918393793,
              "three_lcl": -9.924133914854238,
              "timestamp": "2024-06-26T20:43:27.957229"
            },
        "col_1": {
              "id": "col_1",
              "center": -3.997113080300062,
              "one_ucl": -1.9742384896265417,
              "one_lcl": -6.019987670973582,
              "two_ucl": 0.048636101046978464,
              "two_lcl": -8.042862261647102,
              "three_ucl": 2.071510691720498,
              "three_lcl": -10.065736852320622,
              "timestamp": "2024-06-26T20:43:27.957229"
            },
        "col_3": {
          "id": "col_3",
          "center": -3.937652409303277,
          "one_ucl": -2.0275656995100224,
          "one_lcl": -5.8477391190965315,
          "two_ucl": -0.1174789897167674,
          "two_lcl": -7.757825828889787,
          "three_ucl": 1.7926077200764872,
          "three_lcl": -9.66791253868304,
          "timestamp": "2024-06-26T20:43:27.957230"
        }
      },
      "config": {
        "sample_size": 25,
        "sample": true,
        "name": "test_app",
        "repository": "statworld",
        "version": "0.1.0",
        "alert_config": {
          "rule": {
            "rule": "4 4 4 8 2 4 1 1",
            "zones_to_monitor": [
              "Zone1",
              "Zone2",
              "Zone3",
              "Zone4"
            ]
          },
          "dispatch_type": "Console",
          "schedule": "0 0 0 * * *",
          "features_to_monitor": ["col_1", "col_3"],
          "dispatch_kwargs": {}
        },
        "feature_map": null,
        "targets": [],
        "drift_type": "Spc"
      },
      "scouter_version": "0.3.3"
    }',
    'SPC',
    true,
    '0 0 0 * * *',
    timezone('utc', now() - interval '1 days'),
    timezone('utc', now() - interval '2 days')
  ),
    (
      timezone('utc', now()),
      timezone('utc', now()),
      'test_app',
      'mathworld',
      '0.1.0',
      '{
        "features": {
          "col_2": {
            "id": "col_2",
            "center": -4.090610111507429,
            "one_ucl": -2.146102177058493,
            "one_lcl": -6.035118045956365,
            "two_ucl": -0.20159424260955694,
            "two_lcl": -7.9796259804053005,
            "three_ucl": 1.7429136918393793,
            "three_lcl": -9.924133914854238,
            "timestamp": "2024-06-26T20:43:27.957229"
          },
          "col_1": {
            "id": "col_1",
            "center": -3.997113080300062,
            "one_ucl": -1.9742384896265417,
            "one_lcl": -6.019987670973582,
            "two_ucl": 0.048636101046978464,
            "two_lcl": -8.042862261647102,
            "three_ucl": 2.071510691720498,
            "three_lcl": -10.065736852320622,
            "timestamp": "2024-06-26T20:43:27.957229"
          },
          "col_3": {
            "id": "col_3",
            "center": -3.937652409303277,
            "one_ucl": -2.0275656995100224,
            "one_lcl": -5.8477391190965315,
            "two_ucl": -0.1174789897167674,
            "two_lcl": -7.757825828889787,
            "three_ucl": 1.7926077200764872,
            "three_lcl": -9.66791253868304,
            "timestamp": "2024-06-26T20:43:27.957230"
          }
        },
      "config": {
        "sample_size": 25,
        "sample": true,
        "name": "test_app",
        "repository": "mathworld",
        "version": "0.1.0",
        "alert_config": {
            "dispatch_type": "Console",
            "schedule": "0 0 0 * * *",
            "rule": {
                    "rule": "8 16 4 8 2 4 1 1",
                    "zones_to_monitor": ["Zone 1", "Zone 2", "Zone 3", "Zone 4"]
            },
            "features_to_monitor": [],
            "dispatch_kwargs": {}
        },
        "feature_map": null,
        "targets": [],
        "drift_type": "SPC"
      },
	  "scouter_version": "0.1.0"
      }',
      'SPC',
      false,
      '0 0 0 * * *',
       timezone('utc', now() - interval '1 days'),
      timezone('utc', now() - interval '2 days')
    ),
    (
    timezone('utc', now()),
    timezone('utc', now()),
    'test_app',
    'opsml',
    '0.2.0',
    '{
      "features": {
        "col_2": {
          "id": "col_2",
          "center": -4.090610111507429,
          "one_ucl": -2.146102177058493,
          "one_lcl": -6.035118045956365,
          "two_ucl": -0.20159424260955694,
          "two_lcl": -7.9796259804053005,
          "three_ucl": 1.7429136918393793,
          "three_lcl": -9.924133914854238,
          "timestamp": "2024-06-26T20:43:27.957229"
        },
        "col_1": {
          "id": "col_1",
          "center": -3.997113080300062,
          "one_ucl": -1.9742384896265417,
          "one_lcl": -6.019987670973582,
          "two_ucl": 0.048636101046978464,
          "two_lcl": -8.042862261647102,
          "three_ucl": 2.071510691720498,
          "three_lcl": -10.065736852320622,
          "timestamp": "2024-06-26T20:43:27.957229"
        },
        "col_3": {
          "id": "col_3",
          "center": -3.937652409303277,
          "one_ucl": -2.0275656995100224,
          "one_lcl": -5.8477391190965315,
          "two_ucl": -0.1174789897167674,
          "two_lcl": -7.757825828889787,
          "three_ucl": 1.7926077200764872,
          "three_lcl": -9.66791253868304,
          "timestamp": "2024-06-26T20:43:27.957230"
        }
      },
      "config": {
        "sample_size": 25,
        "sample": true,
        "name": "test_app",
        "repository": "mathworld",
        "version": "0.1.0",
        "alert_config": {
          "rule": {
            "rule": "8 16 4 8 2 4 1 1",
            "zones_to_monitor": [
              "Zone1",
              "Zone2",
              "Zone3",
              "Zone4"
            ]
          },
          "dispatch_type": "Console",
          "schedule": "0 0 0 * * *",
          "features_to_monitor": ["col_1", "col_3"],
          "dispatch_kwargs": {}
        },
        "feature_map": null,
        "targets": [],
        "drift_type": "Spc"
      },
      "scouter_version": "0.3.3"
    }',
    'SPC',
    true,
    '0 0 0 * * *',
    timezone('utc', now() - interval '1 days'),
    timezone('utc', now() - interval '2 days')
  );
   
INSERT INTO drift (created_at, name, repository, feature, value, version)
VALUES
  (timezone('utc', now()), 'test_app', 'statworld', 'col_1', random() - 4, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_2', random() - 4, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_3', random() + 2, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_1', random() - 4, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_2', random() - 4, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_3', random() + 2, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_1', random() - 4, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_2', random() - 4, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_3', random() + 2, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_1', random() - 4, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_2', random() - 4, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_3', random() + 2, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_1', random() - 4, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_2', random() - 4, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_3', random() + 2, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_1', random() - 4, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_2', random() - 4, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_3', random() + 2, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_1', random() - 4, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_2', random() - 4, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_3', random() + 2, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_1', random() - 4, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_2', random() - 4, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_3', random() + 2, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_1', random() - 4, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_2', random() - 4, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_3', random() + 2, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_1', random() - 4, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_2', random() - 4, '0.1.0'),
  (timezone('utc', now()), 'test_app', 'statworld', 'col_3', random() + 2, '0.1.0');



INSERT INTO drift (created_at, name, repository, feature, value, version)
VALUES
  (timezone('utc', now() - interval '1 days'), 'test_app', 'mathworld', 'col_3', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '1 days'), 'test_app', 'mathworld', 'col_1', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '1 days'), 'test_app', 'mathworld', 'col_2', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '1 days'), 'test_app', 'mathworld', 'col_1', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '1 days'), 'test_app', 'mathworld', 'col_2', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '1 days'), 'test_app', 'mathworld', 'col_3', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '1 days'), 'test_app', 'mathworld', 'col_1', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '1 days'), 'test_app', 'mathworld', 'col_2', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '1 days'), 'test_app', 'mathworld', 'col_3', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '1 days'), 'test_app', 'mathworld', 'col_1', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '1 days'), 'test_app', 'mathworld', 'col_2', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '1 days'), 'test_app', 'mathworld', 'col_3', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '1 days'), 'test_app', 'mathworld', 'col_1', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '1 days'), 'test_app', 'mathworld', 'col_2', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '1 days'), 'test_app', 'mathworld', 'col_3', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '2 days'), 'test_app', 'mathworld', 'col_1', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '2 days'), 'test_app', 'mathworld', 'col_2', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '2 days'), 'test_app', 'mathworld', 'col_3', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '2 days'), 'test_app', 'mathworld', 'col_1', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '2 days'), 'test_app', 'mathworld', 'col_2', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '2 days'), 'test_app', 'mathworld', 'col_3', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '2 days'), 'test_app', 'mathworld', 'col_1', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '2 days'), 'test_app', 'mathworld', 'col_2', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '2 days'), 'test_app', 'mathworld', 'col_3', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '2 days'), 'test_app', 'mathworld', 'col_1', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '2 days'), 'test_app', 'mathworld', 'col_2', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '2 days'), 'test_app', 'mathworld', 'col_3', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '2 days'), 'test_app', 'mathworld', 'col_1', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '2 days'), 'test_app', 'mathworld', 'col_2', random() * 20 - 10, '0.1.0'),
  (timezone('utc', now() - interval '2 days'), 'test_app', 'mathworld', 'col_3', random() * 20 - 10, '0.1.0');