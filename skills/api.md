---
name: api
description: REST/GraphQL/OAuth — auth bypass, IDOR, mass assignment, introspection, rate limits.
tags: [api, graphql, oauth, jwt]
---
# API

## Map it
- `curl -s https://$T/api | jq`; `/swagger.json`, `/openapi.json`, `/v2/api-docs`, `/docs`, `/redoc`.
- `kiterunner scan https://$T -w routes.kite` for hidden endpoints.
- Compare `/api/v1` vs `/api/v2` vs `/api/internal` — old versions skip auth.
- Mobile apps leak endpoints: `apktool d app.apk`, grep strings for `https://`; `strings` on IPA.

## Auth
- Try no token, empty token, `Bearer null`, expired token, token from another user/tenant.
- JWT: decode, check `alg`, `kid`, `jku`; try `alg:none` and HS256 with the public key as secret.
- OAuth: `redirect_uri` open redirect, missing `state`, `response_type=token`, scope upgrade, unverified `email`.
- API keys in JS bundles, `/.env`, `config.js`, source maps.

## Objects
- IDOR: increment ids, swap uuid, `?account_id=`, nested `/users/1/orders/2`.
- Mass assignment: add `"role":"admin"`, `"is_admin":true`, `"verified":true` to JSON body.
- Verb tampering and content-type swap (`application/json` → `application/x-www-form-urlencoded`).
- Batch endpoints: `/graphql` aliases, `/batch`, `?ids=1,2,3` — one request, many objects.

## GraphQL
- Introspection: `{"query":"{__schema{types{name fields{name}}}}"}`; `graphql-cop` for misconfig.
- Field suggestions on typos leak schema when introspection is off.
- Aliases for brute force: `{a:login(user:"a",pass:"1") b:login(...)}`.
- Depth/alias DoS, `__typename` errors, mutations without auth.

## Rate limits & logic
- Bypass: `X-Forwarded-For` rotation, case changes in email, unicode normalization, HTTP/2 multiplexing.
- Race: N parallel requests with the same coupon/withdraw (`turbo-intruder`, `ffuf -t 50`, `xargs -P`).
- Negative values, zero, huge ints, float precision, currency rounding.

## Hidden knowledge
- `OPTIONS` on `/api` often returns allowed methods and CORS origins.
- CORS: `Origin: https://evil.com` + `Access-Control-Allow-Credentials: true` = read authenticated data.
- Error verbosity: send `{}`, `[]`, `null`, wrong types — stack traces name frameworks and file paths.
- Check `X-HTTP-Method-Override: DELETE` when only GET/POST is allowed.
- Timestamps: `created_at` order reveals object creation sequence for enumeration.
