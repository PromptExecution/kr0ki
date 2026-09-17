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
