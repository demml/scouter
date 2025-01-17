
from typing import Optional, Dict
from scouter import LogLevel

class KafkaConfig:

    def __init__(self, 
                brokers: Optional[str] = None,
                topic: Optional[str] = None,
                compression_type: Optional[str] = None,
                raise_one_error: bool = False,
                message_timeout_ms: int = 600_000,
                message_max_bytes: int = 2097164,
                log_level: LogLevel = LogLevel.Info,
                config: Dict[str, str] = {}) -> None:
        
        """Kafka configuration to use with the KafkaProducer.

        Args:
            brokers:
                Comma-separated list of Kafka brokers.
                If not provided, the value of the KAFKA_BROKERS environment variable is used.

            topic:
                Kafka topic to publish messages to.
                If not provided, the value of the KAFKA_TOPIC environment variable is used.

            compression_type:
                Compression type to use for messages.
                Default is "gzip".

            raise_on_error:
                Whether to raise an error if message delivery fails.
                Default is True.

            message_timeout_ms:
                Message timeout in milliseconds.
                Default is 600_000.

            message_max_bytes:
                Maximum message size in bytes.
                Default is 2097164.
                
            log_level:
                Log level for the Kafka producer.
                Default is LogLevel.Info.

            config:
                Additional Kafka configuration options. These will be passed to the Kafka producer.
                See https://kafka.apache.org/documentation/#configuration

        """
            
        ...
    