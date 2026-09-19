"""A generic OpenAI-compatible chat-completions client for storyb00k."""

import json
import os
import sys
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

    def chat_completion(self, messages, tools, timeout_hint=None):
        payload = {"model": self.model, "messages": messages}
        if tools:
            payload["tools"] = tools
        _, body = http_call(
            "POST", f"{self.base_url}/chat/completions", json.dumps(payload).encode("utf-8"),
            headers={"Authorization": f"Bearer {self.api_key}", "Content-Type": "application/json"},
        )
        return json.loads(body)
