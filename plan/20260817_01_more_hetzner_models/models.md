curl -s https://inference.hetzner.com/api/v1/models \
  -H "Authorization: Bearer <YOUR_TOKEN>"

Model	Type	Context Length	Modalities
DeepSeek-V4-Flash-0731	MoE, 304B total / 13B active	512.000 tokens	Text
GLM-5.2-NVFP4	MoE, 744B total / 40B active	512.000 tokens	Text
Kimi-K2.7-Code	MoE, 1T total, 32B active	262.144 tokens	Text, Image
Qwen/Qwen3.6-35B-A3B-FP8	MoE, 35B total / 3B active)	262.144 tokens	Text, Image

Python OpenAI Examples

All examples use the OpenAI Python SDK. Install it with:

pip install openai

Initialize the client:

from openai import OpenAI

client = OpenAI(
    base_url="https://inference.hetzner.com/api/v1",
    api_key="<YOUR_TOKEN>",
)

Chat Completions

Send a conversation and receive a model-generated response.

response = client.chat.completions.create(
    model="Qwen/Qwen3.6-35B-A3B-FP8",
    messages=[
        {"role": "system", "content": "You are a helpful assistant."},
        {"role": "user", "content": "Please give me a very short description of Hetzner services!"},
    ]
)

print(response.choices[0].message.content)
