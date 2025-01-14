import pandas as pd
import polars as pl
from scouter.utils.type_converter import PandasConverter, PolarsConverter


def test_pandas_helper(pandas_dataframe: pd.DataFrame) -> None:
    # convert column 0 to category
    pandas_dataframe["column_0"] = pandas_dataframe["column_0"].astype("category")

    # add datetime column
    pandas_dataframe["column_3"] = pd.to_datetime(["2021-01-01" for _ in range(1000)])

    array = PandasConverter(pandas_dataframe).prepare_data()

    assert array.numeric_array is not None
    assert array.string_array is not None

    assert array.numeric_array.shape == (1000, 2)
    assert len(array.string_array[0]) == 1000
    assert len(array.string_array) == 2

    # Pandas converter should not change in place
    assert pandas_dataframe["column_0"].dtype == "category"


def test_polars_helper(polars_dataframe: pl.DataFrame) -> None:
    # convert column 0 to category
    polars_dataframe = polars_dataframe.with_columns(pl.col("column_0").cast(str).cast(pl.Categorical))

    array = PolarsConverter(polars_dataframe).prepare_data()

    assert array.numeric_array is not None
    assert array.string_array is not None

    assert array.numeric_array.shape == (1000, 2)
    assert len(array.string_array[0]) == 1000
    assert len(array.string_array) == 1
