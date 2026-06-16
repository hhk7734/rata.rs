# OpenAPI Components Extraction

## Purpose
Improve the modularity and reusability of the OpenAPI specification (`.rata/openapi.yaml`) by extracting inline definitions into reusable components.

## Goals
- Make the OpenAPI spec easier to maintain and read.
- Ensure consistency in how request bodies and responses are defined across endpoints.

## Non-Goals
- Changing the actual schemas or data structures returned by the API.
- Changing application logic parsing the OpenAPI spec.
