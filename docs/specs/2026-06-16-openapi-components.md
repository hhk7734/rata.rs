# OpenAPI Component Extraction Spec

## Overview
Extract the `/v1/completions` request body and response definitions into reusable OpenAPI components to match the pattern used by `/v1/chat/completions`.

## Architecture & Design
1. **New Request Body Component**
   - Path: `components/requestBodies/CreateCompletionRequest`
   - Structure: Will wrap `$ref: "#/components/schemas/CreateCompletionRequest"` and set `required: true`.

2. **New Response Component**
   - Path: `components/responses/CreateCompletionResponse`
   - Structure: Will wrap `$ref: "#/components/schemas/CreateCompletionResponse"` and provide the description `"A completion response"`.

3. **Endpoint Updates**
   - Path: `/v1/completions`
   - The `requestBody` will be replaced with `$ref: "#/components/requestBodies/CreateCompletionRequest"`.
   - The `responses["200"]` will be replaced with `$ref: "#/components/responses/CreateCompletionResponse"`.

## Testing
- The resulting `.rata/openapi.yaml` should remain valid OpenAPI 3.1.
- No schema semantics should change; endpoints should resolve exactly the same models as before.
