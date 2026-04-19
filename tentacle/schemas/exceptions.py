class TentacleBaseError(Exception):
    """Base class for all Tentacle exceptions."""
    pass


class FetchError(TentacleBaseError):
    """Network timeout or HTTP error during content fetch."""
    pass


class ParseError(TentacleBaseError):
    """DOM parsing or chunking failure."""
    pass


class SectionNotFoundError(TentacleBaseError):
    """Requested section_id not found in document."""
    pass


class SecurityViolationError(TentacleBaseError):
    """SSRF attempt blocked or unsafe URL pattern detected."""
    pass
