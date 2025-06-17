-- Insert initial data into drift_profile
INSERT INTO scouter.drift_profile (created_at, updated_at, name, space, version, profile, drift_type, active, schedule, next_run, previous_run)
VALUES
    (
        timezone('utc', now()),
        timezone('utc', now()),
        'test_app',
        'statworld',
        '0.1.0',
        '{
          "config":{
            "alert_config":{
              "dispatch_config":{
                "Console":{
                  "enabled":true
                }
              },
              "features_to_monitor":[
                "col_1",
                "col_3"
              ],
              "rule":{
                "rule":"8 16 4 8 2 4 1 1",
                "zones_to_monitor":[
                  "Zone1",
                  "Zone2",
                  "Zone3",
                  "Zone4"
                ]
              },
              "schedule":"0 0 0 * * *"
            },
            "drift_type":"Spc",
            "feature_map":{
              "features":{

              }
            },
            "name":"test_app",
            "sample":true,
            "sample_size":25,
            "space":"statworld",
            "version":"0.1.0"
          },
          "features":{
            "col_0":{
              "center":-3.997666447735662,
              "id":"col_0",
              "one_lcl":-6.004629870499931,
              "one_ucl":-1.9907030249713928,
              "three_lcl":-10.01855671602847,
              "three_ucl":2.023223820557146,
              "timestamp":"2025-04-04T00:29:02.218585Z",
              "two_lcl":-8.0115932932642,
              "two_ucl":0.016260397792876358
            },
            "col_1":{
              "center":-4.0109008314933075,
              "id":"col_1",
              "one_lcl":-5.993679615721428,
              "one_ucl":-2.028122047265187,
              "three_lcl":-9.959237184177669,
              "three_ucl":1.9374355211910537,
              "timestamp":"2025-04-04T00:29:02.218594Z",
              "two_lcl":-7.976458399949548,
              "two_ucl":-0.045343263037066706
            },
            "col_2":{
              "center":-3.981840750928434,
              "id":"col_2",
              "one_lcl":-5.982129519823502,
              "one_ucl":-1.9815519820333665,
              "three_lcl":-9.982707057613638,
              "three_ucl":2.0190255557567696,
              "timestamp":"2025-04-04T00:29:02.218595Z",
              "two_lcl":-7.98241828871857,
              "two_ucl":0.01873678686170166
            },
            "col_3":{
              "center":-3.977653738319211,
              "id":"col_3",
              "one_lcl":-5.972708710319746,
              "one_ucl":-1.9825987663186768,
              "three_lcl":-9.962818654320817,
              "three_ucl":2.0075111776823924,
              "timestamp":"2025-04-04T00:29:02.218595Z",
              "two_lcl":-7.96776368232028,
              "two_ucl":0.012456205681858012
            }
          },
          "scouter_version":"0.4.5"
        }',
        'Spc',
        true,
        '0 0 0 * * *',
        timezone('utc', now() - interval '2 days'),
        timezone('utc', now() - interval '3 days')
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
            "space": "mathworld",
            "version": "0.1.0",
            "alert_config": {
                "dispatch_config": {
                  "Console": {
                    "enabled": true
                  }
                },
                "schedule": "0 0 0 * * *",
                "rule": {
                        "rule": "8 16 4 8 2 4 1 1",
                        "zones_to_monitor": ["Zone 1", "Zone 2", "Zone 3", "Zone 4"]
                },
                "features_to_monitor": []
            },
            "feature_map": {
              "features": {}
            },
            "drift_type": "SPC"
          },
        "scouter_version": "0.1.0"
        }',
        'Spc',
        false,
        '0 0 0 * * *',
        timezone('utc', now() - interval '2 days'),
        timezone('utc', now() - interval '3 days')
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
            "space": "mathworld",
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
              "dispatch_config": {
                "Console": {
                  "enabled": true
                }
              },
              "schedule": "0 0 0 * * *",
              "features_to_monitor": ["col_1", "col_3"]
            },
            "feature_map": {
              "features": {}
            },
            "drift_type": "Spc"
          },
          "scouter_version": "0.3.3"
        }',
        'SPC',
        true,
        '0 0 0 * * *',
        timezone('utc', now() - interval '2 days'),
        timezone('utc', now() - interval '3 days')
    );

INSERT INTO scouter.spc_drift (created_at, name, space, feature, value, version)
VALUES
    (timezone('utc', now()), 'test_app', 'statworld', 'col_1', random() - 5, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_2', random() - 4, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_3', random() + 3, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_1', random() - 5, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_2', random() - 4, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_3', random() + 3, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_1', random() - 5, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_2', random() - 4, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_3', random() + 3, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_1', random() - 5, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_2', random() - 4, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_3', random() + 3, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_1', random() - 5, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_2', random() - 4, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_3', random() + 3, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_1', random() - 5, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_2', random() - 4, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_3', random() + 3, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_1', random() - 5, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_2', random() - 4, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_3', random() + 3, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_1', random() - 5, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_2', random() - 4, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_3', random() + 3, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_1', random() - 5, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_2', random() - 4, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_3', random() + 3, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_1', random() - 5, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_2', random() - 4, '0.1.0'),
    (timezone('utc', now()), 'test_app', 'statworld', 'col_3', random() + 3, '0.1.0');



INSERT INTO scouter.spc_drift (created_at, name, space, feature, value, version)
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