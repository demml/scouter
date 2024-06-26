import os

from rusty_logger import JsonConfig, LogConfig, Logger  # type: ignore


class ScouterLogger(Logger):  # type: ignore
    @classmethod
    def get_logger(cls) -> Logger:
        return super().get_logger(
            config=LogConfig(
                stdout=True,
                level=os.environ.get("LOG_LEVEL", "INFO"),
                time_format="[year]-[month]-[day]T[hour repr:24]:[minute]:[second]",
                json_config=JsonConfig(),
            ),
        )
