from __future__ import annotations

from dataclasses import dataclass

from .constants import SAFE_ERROR_DETAILS, ParserErrorCode


@dataclass(slots=True)
class WorkerError(Exception):
    code: ParserErrorCode

    @property
    def safe_message(self) -> str:
        return SAFE_ERROR_DETAILS[self.code][0]

    @property
    def retryable(self) -> bool:
        return SAFE_ERROR_DETAILS[self.code][1]

    def response(self) -> dict[str, object]:
        return {
            "code": self.code.value,
            "message": self.safe_message,
            "retryable": self.retryable,
        }

    def __str__(self) -> str:
        return self.code.value
