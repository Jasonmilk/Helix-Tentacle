import asyncio
from functools import wraps
from typing import Callable

def sync_command(func: Callable) -> Callable:
    """Decorator to turn async CLI command into sync wrapper."""
    @wraps(func)
    def wrapper(*args, **kwargs):
        return asyncio.run(func(*args, **kwargs))
    return wrapper