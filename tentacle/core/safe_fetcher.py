"""Safe HTTP fetcher with retry, SSRF protection, and cookie session support."""

import asyncio
from typing import Optional
from urllib.parse import urlparse

import httpx
from tenacity import (
    retry,
    stop_after_attempt,
    wait_exponential,
    retry_if_exception,
    before_sleep_log,
)

from tentacle.common.config import settings
from tentacle.common.logging import logger
from tentacle.common.cookie_loader import load_cookies
from tentacle.schemas.exceptions import FetchError, SecurityViolationError


class SafeFetcher:
    """
    HTTP client with retry, timeout, SSRF prevention, and cookie session support.

    Embedded mode relies on Tuck gateway for full IP filtering;
    this class provides a fallback local defense.
    """

    # Internal IP ranges to block (local defense only)
    BLOCKED_NETWORKS = [
        "127.0.0.0/8",
        "10.0.0.0/8",
        "172.16.0.0/12",
        "192.168.0.0/16",
        "169.254.0.0/16",
        "::1/128",
        "fc00::/7",
    ]

    def __init__(self) -> None:
        self.timeout = settings.TENTACLE_REQUEST_TIMEOUT
        self.user_agent = settings.TENTACLE_USER_AGENT
        self.allow_private_ips = settings.TENTACLE_ALLOW_PRIVATE_IPS

    async def fetch(self, url: str, cookie_file: Optional[str] = None) -> str:
        """
        Fetch URL content with retry, security checks, and optional cookie injection.

        Args:
            url: Target URL to fetch.
            cookie_file: Cookie file name or path (resolved via cookie_loader).

        Returns:
            Response body as decoded text.

        Raises:
            SecurityViolationError: If URL resolves to a blocked private IP.
            FetchError: If all retries fail or a non-retryable error occurs.
        """
        self._validate_url(url)

        # Load cookies if provided
        cookies = load_cookies(cookie_file) if cookie_file else None

        @retry(
            stop=stop_after_attempt(3),
            wait=wait_exponential(multiplier=1, min=2, max=10),
            retry=retry_if_exception(self._is_retryable),
            before_sleep=before_sleep_log(logger, "WARNING"),
            reraise=True,
        )
        async def _fetch_with_retry() -> str:
            headers = {"User-Agent": self.user_agent}
            async with httpx.AsyncClient(
                timeout=self.timeout,
                cookies=cookies,
                follow_redirects=True,
            ) as client:
                resp = await client.get(url, headers=headers)
                resp.raise_for_status()
                return resp.text

        try:
            return await _fetch_with_retry()
        except Exception as e:
            logger.error("fetch.failed", url=url, error=str(e))
            raise FetchError(f"Failed to fetch URL: {url}") from e

    def _validate_url(self, url: str) -> None:
        """Basic SSRF prevention — block internal IPs."""
        if self.allow_private_ips:
            return

        parsed = urlparse(url)
        hostname = parsed.hostname
        if not hostname:
            return

        import ipaddress
        import socket

        try:
            ip = ipaddress.ip_address(socket.gethostbyname(hostname))
        except Exception:
            # DNS failure will be caught by httpx later
            return

        for net in self.BLOCKED_NETWORKS:
            if ip in ipaddress.ip_network(net):
                raise SecurityViolationError(f"Access to private IP {ip} blocked")

    def _is_retryable(self, exception: Exception) -> bool:
        """Determine if exception should trigger retry."""
        if isinstance(exception, httpx.TimeoutException):
            return True
        if isinstance(exception, httpx.HTTPStatusError):
            return exception.response.status_code in (429, 502, 503, 504)
        if isinstance(exception, httpx.ConnectError):
            return True
        return False