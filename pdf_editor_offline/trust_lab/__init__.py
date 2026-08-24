"""Synthetic, privacy-safe fixtures and verification utilities."""

from .corpus import CORPUS_VERSION, generate_corpus
from .reports import (
    discover_runtime_capabilities,
    inspect_privacy_report,
    public_capabilities_report,
)

__all__ = [
    "CORPUS_VERSION",
    "generate_corpus",
    "discover_runtime_capabilities",
    "inspect_privacy_report",
    "public_capabilities_report",
]
