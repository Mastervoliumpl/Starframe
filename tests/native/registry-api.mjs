import { createServer } from 'node:http';
import { createHash, generateKeyPairSync, randomUUID, sign } from 'node:crypto';
import { crc32 } from 'node:zlib';

const hash = (bytes) => createHash('sha256').update(bytes).digest('hex');
const canonical = (value) =>
  JSON.stringify(
    value && typeof value === 'object'
      ? Array.isArray(value)
        ? value.map((item) => JSON.parse(canonical(item)))
        : Object.fromEntries(
            Object.keys(value)
              .sort()
              .map((key) => [key, JSON.parse(canonical(value[key]))]),
          )
      : value,
  );
function key() {
  const pair = generateKeyPairSync('ed25519');
  const raw = pair.publicKey
    .export({ type: 'spki', format: 'der' })
    .subarray(-32);
  return { ...pair, raw, id: hash(raw) };
}
function envelope(signed, signer) {
  return {
    signed,
    signatures: [
      {
        keyId: signer.id,
        algorithm: 'ed25519',
        signature: sign(
          null,
          Buffer.from(canonical(signed)),
          signer.privateKey,
        ).toString('base64'),
      },
    ],
  };
}
function archive(name, content) {
  const filename = Buffer.from(name);
  const body = Buffer.from(content);
  const checksum = crc32(body);
  const local = Buffer.alloc(30);
  local.writeUInt32LE(0x04034b50);
  local.writeUInt16LE(20, 4);
  local.writeUInt16LE(33, 12);
  local.writeUInt32LE(checksum, 14);
  local.writeUInt32LE(body.length, 18);
  local.writeUInt32LE(body.length, 22);
  local.writeUInt16LE(filename.length, 26);
  const central = Buffer.alloc(46);
  central.writeUInt32LE(0x02014b50);
  central.writeUInt16LE(20, 4);
  central.writeUInt16LE(20, 6);
  central.writeUInt16LE(33, 14);
  central.writeUInt32LE(checksum, 16);
  central.writeUInt32LE(body.length, 20);
  central.writeUInt32LE(body.length, 24);
  central.writeUInt16LE(filename.length, 28);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50);
  end.writeUInt16LE(1, 8);
  end.writeUInt16LE(1, 10);
  end.writeUInt32LE(central.length + filename.length, 12);
  end.writeUInt32LE(local.length + filename.length + body.length, 16);
  return Buffer.concat([local, filename, body, central, filename, end]);
}

export async function registryApi() {
  const root = key();
  const delegated = key();
  const token = 'A'.repeat(43);
  const requests = [];
  const grants = new Map();
  const receipts = new Set();
  const issuedAt = new Date(Date.now() - 60_000).toISOString();
  const expiresAt = new Date(Date.now() + 3_600_000).toISOString();
  let mode = 'online';
  const releases = [2, 1].map((version) => {
    const zip = archive(
      'LJ/lua/fixture.lua',
      `return 'native registry fixture ${version}'\n`,
    );
    return {
      zip,
      release: {
        modId: 1,
        releaseId: randomUUID(),
        versionLabel: `${version}.0.0`,
        artifact: { sha256: hash(zip), bytes: zip.length },
        submissionState: 'approved',
        publicationOrder: version,
        publishedAt: issuedAt,
        createdAt: issuedAt,
        availability: 'available',
        security: { status: 'not_blocked', revision: 1, reason: null },
        metadata: {
          revision: 1,
          state: 'approved',
          testedGameBuild: 'Other:fixture',
          sourceRepository: null,
          releaseNotes: `Fixture version ${version}. <script>Plain text.</script>`,
          dependencies: [],
          dependencyProblems: [],
          installation: {
            schemaVersion: 1,
            kind: 'code',
            loader: 'lua',
            entryPath: 'LJ/lua/fixture.lua',
          },
        },
      },
    };
  });
  const listing = {
    modId: 1,
    modType: 'code',
    name: 'Native registry mod',
    summary: 'Synthetic native discovery and signed install fixture.',
    owner: { displayName: 'Fixture author', avatarUrl: null },
    tags: ['lua'],
    maintained: true,
    replacementModId: null,
    updatedAt: issuedAt,
    latestReleaseId: releases[0].release.releaseId,
    downloads: 0,
    likes: 0,
    archiveBytes: releases[0].zip.length,
    iconId: null,
    publishedAt: issuedAt,
    latestAvailability: 'available',
  };
  const keys = envelope(
    {
      type: 'starframe-registry-keys',
      schemaVersion: 1,
      revision: 1,
      issuedAt,
      expiresAt,
      keys: [
        { keyId: delegated.id, publicKey: delegated.raw.toString('base64') },
      ],
    },
    root,
  );
  const security = envelope(
    {
      type: 'starframe-security',
      schemaVersion: 1,
      revision: 1,
      issuedAt,
      expiresAt,
      complete: true,
      decisions: [],
    },
    delegated,
  );
  const pagination = (pageSize, totalItems) => ({
    page: 1,
    pageSize,
    totalItems,
    totalPages: totalItems ? 1 : 0,
    asOf: issuedAt,
  });
  const server = createServer(async (request, response) => {
    const url = new URL(request.url, 'http://127.0.0.1');
    requests.push({
      method: request.method,
      path: url.pathname,
      query: url.search,
    });
    const json = (data, status = 200) => {
      const body = Buffer.from(JSON.stringify(data));
      response.writeHead(status, {
        'Content-Type': 'application/json',
        'Content-Length': body.length,
        'Cache-Control': 'no-store',
      });
      response.end(body);
    };
    const failure = (status, code, message) =>
      json(
        {
          apiVersion: 1,
          error: {
            code,
            message,
            requestId: randomUUID(),
            retryAfterSeconds: null,
            problems: [],
          },
        },
        status,
      );
    if (mode === 'offline') {
      request.socket.destroy();
      return;
    }
    if (
      request.headers.authorization !== `Bearer ${token}` ||
      mode === 'expired'
    ) {
      failure(401, 'unauthenticated', 'Fixture session ended.');
      return;
    }
    if (url.pathname === '/v1/session' && request.method === 'DELETE') {
      response.writeHead(204);
      response.end();
      return;
    }
    if (url.pathname === '/v1/session') {
      json({
        apiVersion: 1,
        data: {
          accountId: '33333333-3333-4333-8333-333333333333',
          profile: { displayName: 'Fixture user', avatarUrl: null },
          context: 'manager',
          authenticatedAt: issuedAt,
          expiresAt,
          capabilities: ['download_mod'],
          isOwner: false,
        },
      });
      return;
    }
    if (mode === 'restricted') {
      failure(403, 'forbidden', 'Fixture permission denied.');
      return;
    }
    if (url.pathname === '/v1/registry/options') {
      json({
        apiVersion: 1,
        data: {
          tags: [{ id: 'lua', label: 'Lua', groupName: 'Format' }],
          gameBuilds: ['Other:fixture'],
        },
      });
      return;
    }
    if (url.pathname === '/v1/registry/mods') {
      const items = listing.name
        .toLowerCase()
        .includes((url.searchParams.get('query') ?? '').toLowerCase())
        ? [listing]
        : [];
      json({
        apiVersion: 1,
        items,
        pagination: pagination(
          Number(url.searchParams.get('pageSize')),
          items.length,
        ),
      });
      return;
    }
    if (url.pathname === '/v1/registry/mods/1') {
      json({
        apiVersion: 1,
        data: {
          listing,
          description:
            'Native fixture description. <script>Safe text.</script>',
          sourceRepository: null,
          media: { iconId: null, screenshotIds: [] },
          latestRelease: releases[0].release,
        },
      });
      return;
    }
    if (url.pathname === '/v1/registry/mods/1/releases') {
      json({
        apiVersion: 1,
        items: releases.map(({ release: { metadata, ...release } }) => ({
          ...release,
          metadataRevision: metadata.revision,
          testedGameBuild: metadata.testedGameBuild,
        })),
        pagination: pagination(Number(url.searchParams.get('pageSize')), 2),
      });
      return;
    }
    if (url.pathname === '/v1/registry/keys') {
      json(keys);
      return;
    }
    if (url.pathname === '/v1/registry/security') {
      json(security);
      return;
    }
    const selected = releases.find(({ release }) =>
      url.pathname.startsWith(`/v1/registry/releases/${release.releaseId}`),
    );
    if (selected) {
      const { release, zip } = selected;
      const suffix = url.pathname.slice(
        `/v1/registry/releases/${release.releaseId}`.length,
      );
      if (!suffix) {
        json({ apiVersion: 1, data: release });
        return;
      }
      if (suffix === '/manifest') {
        const signed = envelope(
          {
            type: 'starframe-release',
            schemaVersion: 2,
            revision: 1,
            issuedAt,
            expiresAt:
              mode === 'stale'
                ? new Date(Date.now() - 1000).toISOString()
                : expiresAt,
            securityRevision: 1,
            release,
          },
          delegated,
        );
        if (mode === 'tampered')
          signed.signed.release = {
            ...release,
            artifact: { ...release.artifact, sha256: 'f'.repeat(64) },
          };
        json(signed);
        return;
      }
      let bytes = '';
      for await (const chunk of request) bytes += chunk;
      const body = JSON.parse(bytes || '{}');
      if (suffix === '/downloads') {
        if (
          body.sha256 !== release.artifact.sha256 ||
          body.metadataRevision !== 1 ||
          body.securityRevision !== 1
        ) {
          failure(409, 'security_stale', 'Fixture exact request changed.');
          return;
        }
        const downloadId = randomUUID();
        grants.set(downloadId, selected);
        json(
          {
            apiVersion: 1,
            data: {
              downloadId,
              reference: {
                modId: 1,
                releaseId: release.releaseId,
                sha256: release.artifact.sha256,
              },
              bytes: zip.length,
              metadataRevision: 1,
              securityRevision: 1,
              contentPath: `/v1/downloads/${downloadId}/content`,
              expiresAt: new Date(Date.now() + 600_000).toISOString(),
            },
          },
          201,
        );
        return;
      }
      if (suffix === '/receipts') {
        if (
          !grants.has(body.downloadId) ||
          body.sha256 !== release.artifact.sha256 ||
          body.bytes !== zip.length
        ) {
          failure(409, 'conflict', 'Fixture receipt mismatch.');
          return;
        }
        const counted = !receipts.has(release.releaseId);
        receipts.add(release.releaseId);
        json({
          apiVersion: 1,
          data: { releaseId: release.releaseId, counted },
        });
        return;
      }
    }
    const content = /^\/v1\/downloads\/([^/]+)\/content$/.exec(url.pathname);
    if (content && grants.has(content[1])) {
      const { release, zip } = grants.get(content[1]);
      response.writeHead(200, {
        'Content-Type': 'application/zip',
        'Content-Length': zip.length,
        'Accept-Ranges': 'bytes',
        ETag: `"sha256-${release.artifact.sha256}"`,
        'Cache-Control': 'no-store',
      });
      response.end(zip);
      return;
    }
    failure(404, 'not_found', 'Unknown fixture route.');
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const origin = `http://127.0.0.1:${server.address().port}`;
  return {
    config: {
      schemaVersion: 1,
      apiUrl: `${origin}/v1`,
      websiteUrl: `${origin}/`,
      rootPublic: [...root.raw],
    },
    credential: { schemaVersion: 1, token, expiresAt },
    releases: releases.map(({ release }) => release),
    requests,
    receipts,
    mode(value) {
      mode = value;
    },
    async close() {
      server.closeAllConnections();
      await new Promise((resolve) => server.close(resolve));
    },
  };
}
