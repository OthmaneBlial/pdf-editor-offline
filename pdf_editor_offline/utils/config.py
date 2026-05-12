import os


class Config:
    DEBUG = os.getenv("DEBUG", "False").lower() in ("true", "1", "yes")
    LOG_LEVEL = os.getenv("LOG_LEVEL", "INFO")
