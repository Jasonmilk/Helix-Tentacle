from pydantic_settings import BaseSettings, SettingsConfigDict

class Settings(BaseSettings):
    # 默认指向 Tuck 网关
    tuck_url: str = "http://127.0.0.1:8000/v1/chat/completions"
    tuck_api_key: str = "" # 如果 Tuck 开启了鉴权，这里填入
    
    # 触手默认使用的轻量级模型（比如 2B 或 8B 模型）
    default_probe_model: str = "qwen2.5-2b" # 请替换为你接入 Tuck 的实际轻量模型名
    
    # 探查参数
    sample_size: int = 4000  # 每次探查截取的最大字符数
    
    model_config = SettingsConfigDict(env_file=".env", env_file_encoding="utf-8")

settings = Settings()
