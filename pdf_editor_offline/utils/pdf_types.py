from dataclasses import dataclass
from typing import Optional


@dataclass
class Point:
    x: float
    y: float


@dataclass
class Rectangle:
    x0: float
    y0: float
    x1: float
    y1: float


@dataclass
class Metadata:
    title: Optional[str] = None
    author: Optional[str] = None
    subject: Optional[str] = None
    creator: Optional[str] = None
    producer: Optional[str] = None
    creation_date: Optional[str] = None
    mod_date: Optional[str] = None
