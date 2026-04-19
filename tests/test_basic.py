import pytest
from tentacle.common.config import settings


def test_settings_load():
    """Test that settings load correctly."""
    assert settings.TENTACLE_MODE in ["embedded", "standalone"]
    assert settings.TENTACLE_PORT > 0
    assert settings.TENTACLE_REQUEST_TIMEOUT > 0


def test_version():
    """Test version is correct."""
    from tentacle import __version__
    assert __version__ == "2.1.0"
