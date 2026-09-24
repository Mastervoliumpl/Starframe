# Registry downloads

Issue #85 is in progress against the committed website delivery contract `fcd81be6667e2595166698178d5f55074c5ee92a`. The website checkout is read-only to this task; its current #101 edits are unfinished and were not used to define this transfer.

The storage boundary derives a download identity only from saved, fresh signatures for the requested ModID and ReleaseID. It checks the retained hash decisions before returning the exact hash, size, approved metadata revision and security revision. The authenticated grant request sends those values. The response must repeat them, carry a relative content path for its DownloadID and have a short valid expiry. A 2 GiB archive cap matches the website upload limit.

The content reader uses a separate HTTP client without the JSON request's 30-second total timeout. It streams into a caller-owned staging file. It accepts only the website SHA-256 ETag, explicit size and range headers, a complete 200 response or a matching 206 suffix. A 200 response resets a partial file. A 416 remains a distinct error for the caller to handle. Completion reads the whole staged file in bounded chunks and checks the signed SHA-256 before returning success. A corrupt or partial file receives no success result.

Loopback tests cover a fresh transfer, a suffix resume, a full restart of a partial file, changed bytes, 416 and cancellation. The grant fixture checks the exact identity and rejects an external content URL. No website credential, live archive, real game or installer was used.

Grant renewal, server-delivered-prefix recovery, account/session binding across restarts, receipt persistence and retry, queue integration, post-transfer trust rechecks, and a representative large-transfer check remain open. No registry archive is promoted or installed by this slice.
