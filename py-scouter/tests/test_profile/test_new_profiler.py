import pandas as pd
from scouter import DataType, RustScouterProfiler
import polars as pl
from numpy.typing import NDArray

def test_data_profile_pandas_mixed_type(
    pandas_dataframe_multi_type: pd.DataFrame,
):

    profile = RustScouterProfiler()
    profile.create_data_profile(pandas_dataframe_multi_type, DataType.Pandas)
    a
    
def test_data_profile_polars_mixed_type(
    polars_dataframe_multi_dtype: pl.DataFrame,
):

    profile = RustScouterProfiler()
    profile.create_data_profile(polars_dataframe_multi_dtype, DataType.Polars)
    a

def test_data_profile_arrow_mixed_type(
    polars_dataframe_multi_dtype: pl.DataFrame,
):
    arrow_table = polars_dataframe_multi_dtype.to_arrow()
    profile = RustScouterProfiler()
    profile.create_data_profile(arrow_table, DataType.Arrow)
    a
    
def test_data_profile_array(
    array: NDArray
):
    
    profile = RustScouterProfiler()
    profile.create_data_profile(array, DataType.Numpy)
    a
    
    #ndarray 
    #Table
    