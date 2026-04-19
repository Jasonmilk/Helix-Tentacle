"""Shared CLI utilities for argument parsing."""

from typing import List, Optional, Dict


def parse_comma_list(value: Optional[str]) -> List[str]:
    """
    Parse a comma-separated string into a list of trimmed non-empty tokens.

    Args:
        value: Comma-separated string or None.

    Returns:
        List of trimmed tokens. Empty list if value is None or empty.
    """
    if not value:
        return []
    return [token.strip() for token in value.split(",") if token.strip()]


def parse_boost_dict(value: Optional[str]) -> Dict[str, float]:
    """
    Parse a boost specification string into a dictionary.

    Expected format: "word:2.0,another:1.5"

    Args:
        value: Boost specification string or None.

    Returns:
        Dictionary mapping words to float weights. Empty dict if invalid input.
    """
    result: Dict[str, float] = {}
    if not value:
        return result

    for pair in value.split(","):
        pair = pair.strip()
        if ":" not in pair:
            continue
        word, weight_str = pair.split(":", 1)
        word = word.strip()
        if not word:
            continue
        try:
            weight = float(weight_str.strip())
            if weight > 0:
                result[word] = weight
        except ValueError:
            continue
    return result