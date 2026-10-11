# OpenAI vision adapter

The first provider uses the OpenAI Responses API at the fixed HTTPS endpoint `https://api.openai.com/v1/responses`, with `gpt-4.1-mini` as the default configurable model. The provider implements the common vision interface; naming and UI use application metadata and safe error categories.

Requests accept PNG/JPEG up to 8 MiB, with a maximum 512-character optional context hint. Images are sent as base64 data URLs at low detail. Output uses a strict JSON schema with nullable app/topic/detail/confidence fields and a 256-token output cap. Labels are limited to 128 characters, reject control characters, and require at least one useful label. Confidence must be finite and between zero and one. Responses are bounded to 64 KiB. Refusals, incomplete and malformed responses are recoverable failures.

Credentials are loaded from Windows Credential Manager. HTTP Authorization headers are marked sensitive. No image, key, raw response, or underlying HTTP error is logged or returned to the UI. Redirects and automatic transport retries are disabled. Authentication and invalid output do not retry. Timeout, rate-limit and transient network/server failures receive at most one retry after 200 ms. Each HTTP attempt has a four-second limit inside an eight-second overall budget, with a two-second connection timeout. Saved consent is checked before each request; retries reload the credential.

Only the Rust backend needs outbound HTTPS access to `api.openai.com:443`; there is no frontend HTTP permission. Uploads require the saved cloud mode, enablement and explicit consent. Provider settings cannot select another endpoint.

As verified against official documentation on 2026-10-11, GPT-4.1 mini lists $0.40 per million input text tokens and $1.60 per million output tokens; image input is billed by the provider's vision token rules. API costs are charged to the configured account. Requests set `store:false`. This disables stored response state but does not promise zero retention: default abuse-monitoring retention can be up to 30 days, with policy exceptions. Use the provider's current account data controls for additional limits.

Sources: [image inputs](https://developers.openai.com/api/docs/guides/images-vision), [structured output](https://developers.openai.com/api/docs/guides/structured-outputs), [model pricing](https://developers.openai.com/api/docs/models/gpt-4.1-mini), [data controls](https://developers.openai.com/api/docs/guides/your-data), [HTTP client](https://docs.rs/reqwest/latest/reqwest/).

The regular suite uses fake HTTP responses and loopback HTTP servers with dummy keys. The opt-in test `live_synthetic_windows_vision_check` uploads only `tests/fixtures/ocr-text.png`, using the saved Windows test credential. Run explicitly with `cargo test -j 1 --test openai_contract live_synthetic_windows_vision_check -- --ignored`; never run it without user opt-in.
