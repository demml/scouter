In addition to monitoring, `Scouter` also provides data profiling tools to create feature distribution profiles to associate with your data.

## Supported Data Types
Scouter supports a variety of data types, including:

- <span class="text-primary">**Pandas DataFrames**</span>: Scouter can handle Pandas DataFrames, making it easy to integrate with existing data processing pipelines.
- <span class="text-primary">**Numpy Arrays**</span>: Out of the box support for 2D arrays.
- <span class="text-primary">**Polars DataFrames**</span>: For users who prefer Polars, Scouter supports this data format as well, allowing for efficient data processing and analysis.

## Create a Profile

```python
import numpy as np
import pandas as pd
from scouter import DataProfile, DataProfiler  # type: ignore[attr-defined]


def generate_data() -> pd.DataFrame:
    """Create a fake data frame for testing"""
    n = 10_000
    X_train = np.random.normal(-4, 2.0, size=(n, 4))
    col_names = []
    for i in range(0, X_train.shape[1]):
        col_names.append(f"feature_{i}")
    X = pd.DataFrame(X_train, columns=col_names)

    # create string column (with 10 unique values)
    X["categorical_feature"] = np.random.choice(
        ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"], size=n
    )

    return X


data = generate_data()

# create data profiler
profiler = DataProfiler()

# create data profile
profile: DataProfile = profiler.create_data_profile(data)

# Save (provide a path or leave blank)
profile.save_to_json()

print(profile)
```

???success "Output"

    ```json
        {
        "features": {
            "categorical_feature": {
            "id": "categorical_feature",
            "numeric_stats": null,
            "string_stats": {
                "distinct": {
                "count": 10,
                "percent": 0.1
                },
                "char_stats": {
                "min_length": 1,
                "max_length": 1,
                "median_length": 1,
                "mean_length": 1.0
                },
                "word_stats": {
                "words": {
                    "d": {
                    "count": 1021,
                    "percent": 0.1021
                    },
                    "b": {
                    "count": 998,
                    "percent": 0.0998
                    },
                    "i": {
                    "count": 989,
                    "percent": 0.0989
                    },
                    "c": {
                    "count": 1015,
                    "percent": 0.1015
                    },
                    "f": {
                    "count": 1007,
                    "percent": 0.1007
                    },
                    "a": {
                    "count": 987,
                    "percent": 0.0987
                    },
                    "h": {
                    "count": 982,
                    "percent": 0.0982
                    },
                    "g": {
                    "count": 988,
                    "percent": 0.0988
                    },
                    "j": {
                    "count": 1020,
                    "percent": 0.102
                    },
                    "e": {
                    "count": 993,
                    "percent": 0.0993
                    }
                }
                }
            },
            "timestamp": "2025-04-28T18:19:33.987594Z",
            "correlations": null
            },
            "feature_0": {
            "id": "feature_0",
            "numeric_stats": {
                "mean": -4.003965640199899,
                "stddev": 2.0178515177174448,
                "min": -11.084430177884318,
                "max": 3.375571168173134,
                "distinct": {
                "count": 10000,
                "percent": 1.0
                },
                "quantiles": {
                "q25": -5.348060765665304,
                "q50": -3.9960621892367625,
                "q75": -2.6379431350112723,
                "q99": 0.7695557247479057
                },
                "histogram": {
                "bins": [
                    -11.084430177884318,
                    -10.361430110581445,
                    -9.638430043278573,
                    -8.9154299759757,
                    -8.192429908672828,
                    -7.469429841369955,
                    -6.7464297740670816,
                    -6.023429706764209,
                    -5.300429639461337,
                    -4.577429572158464,
                    -3.8544295048555917,
                    -3.1314294375527183,
                    -2.408429370249845,
                    -1.6854293029469734,
                    -0.9624292356441,
                    -0.2394291683412284,
                    0.48357089896164496,
                    1.2065709662645183,
                    1.92957103356739,
                    2.6525711008702633
                ],
                "bin_counts": [
                    8,
                    14,
                    53,
                    120,
                    228,
                    444,
                    706,
                    1006,
                    1300,
                    1399,
                    1404,
                    1159,
                    914,
                    599,
                    352,
                    153,
                    81,
                    39,
                    17,
                    4
                ]
                }
            },
            "string_stats": null,
            "timestamp": "2025-04-28T18:19:34.035699Z",
            "correlations": null
            },
            "feature_1": {
            "id": "feature_1",
            "numeric_stats": {
                "mean": -3.997238711617275,
                "stddev": 1.9969030204031852,
                "min": -11.2143596931969,
                "max": 3.5233542660216592,
                "distinct": {
                "count": 10000,
                "percent": 1.0
                },
                "quantiles": {
                "q25": -5.331856994963132,
                "q50": -3.9985892307105217,
                "q75": -2.6720412739258803,
                "q99": 0.7018279512701495
                },
                "histogram": {
                "bins": [
                    -11.2143596931969,
                    -10.477473995235973,
                    -9.740588297275044,
                    -9.003702599314117,
                    -8.266816901353188,
                    -7.529931203392261,
                    -6.7930455054313335,
                    -6.056159807470405,
                    -5.319274109509477,
                    -4.582388411548549,
                    -3.8455027135876207,
                    -3.1086170156266935,
                    -2.371731317665766,
                    -1.634845619704837,
                    -0.8979599217439098,
                    -0.16107422378298075,
                    0.5758114741779465,
                    1.3126971721388738,
                    2.049582870099803,
                    2.78646856806073
                ],
                "bin_counts": [
                    7,
                    18,
                    42,
                    93,
                    218,
                    425,
                    706,
                    1015,
                    1345,
                    1431,
                    1430,
                    1216,
                    866,
                    580,
                    340,
                    158,
                    72,
                    20,
                    13,
                    5
                ]
                }
            },
            "string_stats": null,
            "timestamp": "2025-04-28T18:19:34.035702Z",
            "correlations": null
            },
            "feature_2": {
            "id": "feature_2",
            "numeric_stats": {
                "mean": -3.9985933460650362,
                "stddev": 1.9872554303247896,
                "min": -11.298088188584437,
                "max": 3.7792120832159224,
                "distinct": {
                "count": 10000,
                "percent": 1.0
                },
                "quantiles": {
                "q25": -5.3669588658728955,
                "q50": -3.991356640189083,
                "q75": -2.6604527797988693,
                "q99": 0.716291352460563
                },
                "histogram": {
                "bins": [
                    -11.298088188584437,
                    -10.54422317499442,
                    -9.7903581614044,
                    -9.036493147814383,
                    -8.282628134224364,
                    -7.528763120634347,
                    -6.774898107044329,
                    -6.021033093454311,
                    -5.267168079864293,
                    -4.513303066274275,
                    -3.7594380526842563,
                    -3.005573039094239,
                    -2.2517080255042217,
                    -1.4978430119142025,
                    -0.7439779983241852,
                    0.00988701526583391,
                    0.7637520288558513,
                    1.5176170424458686,
                    2.2714820560358877,
                    3.025347069625905
                ],
                "bin_counts": [
                    3,
                    11,
                    36,
                    109,
                    222,
                    423,
                    744,
                    1114,
                    1321,
                    1487,
                    1454,
                    1193,
                    853,
                    530,
                    284,
                    120,
                    52,
                    29,
                    13,
                    2
                ]
                }
            },
            "string_stats": null,
            "timestamp": "2025-04-28T18:19:34.035703Z",
            "correlations": null
            },
            "feature_3": {
            "id": "feature_3",
            "numeric_stats": {
                "mean": -3.996914164800711,
                "stddev": 1.9930422779448296,
                "min": -11.140741264279484,
                "max": 2.876859924322919,
                "distinct": {
                "count": 10000,
                "percent": 1.0
                },
                "quantiles": {
                "q25": -5.332108894837108,
                "q50": -3.9894123025713193,
                "q75": -2.639771809727505,
                "q99": 0.531352688455816
                },
                "histogram": {
                "bins": [
                    -11.140741264279484,
                    -10.439861204849365,
                    -9.738981145419244,
                    -9.038101085989123,
                    -8.337221026559003,
                    -7.636340967128883,
                    -6.935460907698763,
                    -6.234580848268643,
                    -5.533700788838523,
                    -4.832820729408403,
                    -4.131940669978283,
                    -3.4310606105481627,
                    -2.7301805511180426,
                    -2.0293004916879234,
                    -1.3284204322578024,
                    -0.6275403728276814,
                    0.07333968660243784,
                    0.774219746032557,
                    1.475099805462678,
                    2.175979864892799
                ],
                "bin_counts": [
                    3,
                    22,
                    37,
                    79,
                    218,
                    385,
                    557,
                    870,
                    1214,
                    1333,
                    1422,
                    1216,
                    1002,
                    759,
                    422,
                    251,
                    146,
                    42,
                    16,
                    6
                ]
                }
            },
            "string_stats": null,
            "timestamp": "2025-04-28T18:19:34.035704Z",
            "correlations": null
            }
        }
    }
    ```

