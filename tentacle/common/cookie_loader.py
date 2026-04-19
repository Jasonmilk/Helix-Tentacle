"""Cookie file resolution and loading utilities."""

from pathlib import Path
from http.cookiejar import MozillaCookieJar

from tentacle.common.logging import logger
from tentacle.schemas.exceptions import FetchError


def resolve_cookie_path(cookie_spec: str) -> Path:
    """
    Resolve cookie specification to an absolute file path.

    Resolution order:
    1. Absolute path → return as is.
    2. Relative path with directory separator → resolve from CWD.
    3. Simple filename → search in:
       - ./cookies/
       - ~/.config/tentacle/cookies/
    """
    spec_path = Path(cookie_spec)

    if spec_path.is_absolute():
        return spec_path

    if "/" in cookie_spec or "\\" in cookie_spec:
        return Path.cwd() / spec_path

    search_paths = [
        Path.cwd() / "cookies" / cookie_spec,
        Path.home() / ".config" / "tentacle" / "cookies" / cookie_spec,
    ]

    for path in search_paths:
        if path.exists():
            return path

    return search_paths[0]


def load_cookies(cookie_spec: str) -> MozillaCookieJar:
    """
    Load Netscape format cookies from a resolved path.

    Args:
        cookie_spec: Cookie file name or path.

    Returns:
        MozillaCookieJar object containing the loaded cookies.

    Raises:
        FetchError: If the file cannot be read or parsed.
    """
    cookie_path = resolve_cookie_path(cookie_spec)
    if not cookie_path.exists():
        raise FetchError(f"Cookie file not found: {cookie_path}")

    jar = MozillaCookieJar()
    try:
        jar.load(str(cookie_path), ignore_discard=True, ignore_expires=True)
    except Exception as e:
        raise FetchError(f"Failed to load cookie file: {e}") from e

    logger.debug("cookies.loaded", path=str(cookie_path), count=len(jar))
    return jar