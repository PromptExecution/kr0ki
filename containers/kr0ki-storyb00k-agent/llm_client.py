"""A generic OpenAI-compatible chat-completions client for storyb00k."""

import json
import os
import sys
import time
from pathlib import Path

try:
    from manifest_dispatch import http_call
except ModuleNotFoundError:  # local source-tree tests; the image copies the module beside us
    sys.path.insert(0, str(Path(__file__).parents[1] / "kr0ki-mcp"))
    from manifest_dispatch import http_call


class ConfigError(Exception):
    pass


class OpenAICompatibleClient:
    def __init__(self, api_key, base_url, model="default"):
        self.api_key = api_key
        self.base_url = base_url.rstrip("/")
        self.model = model
        self._vision = None  # (value, checked_at)

    @classmethod
    def from_env(cls):
        api_key = os.environ.get("OPENAI_API_KEY")
        base_url = os.environ.get("OPENAI_API_URL")
        if not api_key or not base_url:
            raise ConfigError("OPENAI_API_KEY and OPENAI_API_URL must both be set")
        return cls(api_key, base_url, model=cls.discover_model(base_url, api_key))

    @staticmethod
    def discover_model(base_url, api_key):
        """Use the first model the endpoint advertises; fall back to 'default'.

        Cheap metadata call (GET /models) — never an inference request.
        """
        try:
            _, body = http_call(
                "GET", f"{base_url.rstrip('/')}/models", None,
                headers={"Authorization": f"Bearer {api_key}"},
            )
            data = json.loads(body).get("data") or []
            return data[0]["id"] if data else "default"
        except Exception:
            return "default"

    def vision_enabled(self, ttl=60):
        """False only when the endpoint says it cannot take images (llama.cpp `/props` -> modalities.vision == false).

        Anything else (no /props, a hosted API, an error) is treated as capable: sending an image to a text-only model is a
        hard 500, so the one case we can positively detect is the one we avoid. Cached for `ttl` seconds."""
        if self._vision and time.monotonic() - self._vision[1] < ttl:
            return self._vision[0]
        value = True
        try:
            root = self.base_url[:-3] if self.base_url.endswith("/v1") else self.base_url
            _, body = http_call("GET", f"{root}/props", None, headers={"Authorization": f"Bearer {self.api_key}"})
            if (json.loads(body).get("modalities") or {}).get("vision") is False:
                value = False
        except Exception:
            pass
        self._vision = (value, time.monotonic())
        return value

    def chat_completion(self, messages, tools, timeout_hint=None):
        payload = {"model": self.model, "messages": messages}
        if tools:
            payload["tools"] = tools
        _, body = http_call(
            "POST", f"{self.base_url}/chat/completions", json.dumps(payload).encode("utf-8"),
            headers={"Authorization": f"Bearer {self.api_key}", "Content-Type": "application/json"},
        )
        return json.loads(body)
