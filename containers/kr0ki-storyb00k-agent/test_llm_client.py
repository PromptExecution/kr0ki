import json
import os
import unittest
from unittest.mock import patch

import llm_client


class LlmClientTest(unittest.TestCase):
    def test_client_reads_openai_compatible_config_from_env(self):
        with patch.dict(os.environ, {"OPENAI_API_KEY": "sk-test", "OPENAI_API_URL": "http://example.invalid/v1"}, clear=True):
            client = llm_client.OpenAICompatibleClient.from_env()
        self.assertEqual(client.api_key, "sk-test")
        self.assertEqual(client.base_url, "http://example.invalid/v1")

    def test_missing_config_is_explicit(self):
        with patch.dict(os.environ, {}, clear=True):
            with self.assertRaises(llm_client.ConfigError):
                llm_client.OpenAICompatibleClient.from_env()

    @patch("llm_client.http_call")
    def test_completion_posts_json_to_the_standard_endpoint(self, http_call):
        http_call.return_value = ("application/json", json.dumps({"choices": [{"message": {"content": "hello"}}]}).encode())
        result = llm_client.OpenAICompatibleClient("sk-test", "http://example.invalid/v1/").chat_completion([{"role": "user", "content": "hi"}], [])
        method, url, body = http_call.call_args.args[:3]
        self.assertEqual((method, url), ("POST", "http://example.invalid/v1/chat/completions"))
        self.assertEqual(json.loads(body)["messages"][0]["content"], "hi")
        self.assertEqual(result["choices"][0]["message"]["content"], "hello")


class VisionCapabilityTest(unittest.TestCase):
    def client(self, base="http://h:8001/v1"):
        return llm_client.OpenAICompatibleClient("k", base)

    @patch("llm_client.http_call")
    def test_a_text_only_llama_cpp_server_is_detected_from_props(self, http_call):
        http_call.return_value = ("application/json", json.dumps({"modalities": {"vision": False, "audio": False}}).encode())
        c = self.client()
        self.assertFalse(c.vision_enabled())
        self.assertEqual(http_call.call_args.args[:2], ("GET", "http://h:8001/props"))  # /props lives at the root, not under /v1

    @patch("llm_client.http_call")
    def test_vision_capable_servers_and_unknown_endpoints_are_treated_as_capable(self, http_call):
        http_call.return_value = ("application/json", json.dumps({"modalities": {"vision": True}}).encode())
        self.assertTrue(self.client().vision_enabled())
        http_call.side_effect = OSError("404 no /props on a hosted API")
        self.assertTrue(self.client("https://api.example/v1").vision_enabled())
        http_call.side_effect = None
        http_call.return_value = ("application/json", b"not json")
        self.assertTrue(self.client().vision_enabled())

    @patch("llm_client.http_call")
    def test_the_answer_is_cached_for_a_minute(self, http_call):
        http_call.return_value = ("application/json", json.dumps({"modalities": {"vision": False}}).encode())
        c = self.client()
        c.vision_enabled(); c.vision_enabled(); c.vision_enabled()
        self.assertEqual(http_call.call_count, 1)
        c.vision_enabled(ttl=0)
        self.assertEqual(http_call.call_count, 2)
