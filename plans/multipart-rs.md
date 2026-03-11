# multipart-rs integration plan

> `multipart-rs` — Multipart form data parsing.

## What it does

Handles multipart/form-data encoding and decoding, used for HTTP file uploads.

## Where it fits in miniR

### 1. HTTP file uploads

If miniR implements `httr::POST()` with file upload support, multipart encoding
is needed to construct the request body.

### 2. Limited R relevance

Most R HTTP usage is GET requests or JSON POST bodies. Multipart is only needed
for file upload APIs.

## Recommendation

**Add only when implementing HTTP POST with file upload.** Very low priority
relative to other builtins.

**Effort:** Trivial to add, used in one place.
