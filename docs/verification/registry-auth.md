# Manager authentication boundary

Issue #83 is in progress against the website v1 contract last changed at `fcd81be6667e2595166698178d5f55074c5ee92a`. The read-only website main reference was `ba8803aea4b6a067c1f9c60d5eb3ac33c7358567` when this work started.

The Rust client creates a random PKCE S256 verifier, checks the returned website verification link and code, and enforces five seconds between exchange polls. It accepts one bearer response and keeps the verifier and credential out of frontend response types. Windows Credential Manager stores the opaque token for the current user. A saved credential is inspected with `GET /v1/session`; a revoked or expired session is removed locally. Sign-out deletes the local credential before remote revocation and reports remote failure separately. A network failure during inspection retains the credential for a later retry. Local mods and game operations do not depend on these online calls.

Loopback HTTP tests cover PKCE, pending and successful polls, a foreign verification link, session restore, 401 removal and failed remote sign-out. The Windows credential test uses a unique synthetic target and token. No Steam account, deployed website or live credential was used. The desktop commands and UI, cancellation/expiry coverage, and hosted checks remain before #83 can close.
