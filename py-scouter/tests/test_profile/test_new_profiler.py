import pandas as pd
from scouter import DataType, RustScouterProfiler


def test_data_profile_pandas_mixed_type(
    pandas_dataframe_multi_type: pd.DataFrame,
):

    profile = RustScouterProfiler()
    profile.create_data_profile(pandas_dataframe_multi_type, DataType.Pandas)
    a
