import type { RegistryTransport } from '../lib/registry';
import type { ModSummary, Release, ModDetail } from '../lib/generated/registry';

const date = '2026-09-24T00:00:00Z';
export const fixtureReleaseId = (id: number, order = 2) =>
  `${String(id).padStart(8, '0')}-1111-4111-8111-${String(order).padStart(12, '0')}`;
export function fixtureRelease(id: number, order = 2): Release {
  return {
    modId: id,
    releaseId: fixtureReleaseId(id, order),
    versionLabel: `${order}.0.0`,
    artifact: { sha256: String(id % 10).repeat(64), bytes: 1024 * 1024 },
    submissionState: 'approved',
    publicationOrder: order,
    publishedAt: date,
    createdAt: date,
    availability: id === 3 ? 'blocked' : 'available',
    security: {
      status: id === 3 ? 'blocked' : 'not_blocked',
      revision: 1,
      reason: id === 3 ? 'Confirmed fixture security block.' : null,
    },
    metadata: {
      revision: 1,
      state: 'approved',
      testedGameBuild: 'Steam:123',
      sourceRepository: 'https://github.com/example/fixture',
      releaseNotes:
        order === 2
          ? 'Updated fixture release. <script>Plain text only.</script>'
          : 'Earlier fixture release.',
      dependencies: [],
      dependencyProblems: [],
      installation: {
        schemaVersion: 1,
        kind: 'code',
        loader: 'lua',
        entryPath: 'LJ/lua/mod.lua',
      },
    },
  };
}
export function fixtureMod(id: number): ModSummary {
  return {
    modId: id,
    modType: id % 3 === 0 ? 'ai' : id % 2 === 0 ? 'map' : 'code',
    name: `Mod fixture ${id}`,
    summary: 'An author-supplied description of this fixture mod.',
    owner: { displayName: `Author ${id}`, avatarUrl: null },
    tags: id % 2 === 0 ? ['tools'] : ['lua'],
    maintained: id !== 2,
    replacementModId: id === 2 ? 1 : null,
    updatedAt: date,
    latestReleaseId: fixtureReleaseId(id),
    downloads: 100 - id,
    likes: id,
    archiveBytes: 1024 * 1024,
    iconId: id === 1 ? '88888888-1111-4111-8111-111111111111' : null,
    publishedAt: date,
    latestAvailability: id === 3 ? 'blocked' : 'available',
  };
}
export function fixtureDetail(id: number): ModDetail {
  return {
    listing: fixtureMod(id),
    description:
      'Fixture detail. <script>Descriptions stay plain text.</script>',
    sourceRepository: 'https://github.com/example/fixture',
    media: {
      iconId: null,
      screenshotIds: id === 1 ? ['88888888-1111-4111-8111-222222222222'] : [],
    },
    latestRelease: fixtureRelease(id),
  };
}
export function fixtureRegistry(): RegistryTransport {
  const params = new URLSearchParams(location.search);
  const mode = params.get('registry');
  const mods = Array.from(
    { length: params.has('large') ? 1200 : 16 },
    (_, index) => fixtureMod(index + 1),
  );
  const pagination = (page: number, pageSize: number, totalItems: number) => ({
    page,
    pageSize,
    totalItems,
    totalPages: Math.ceil(totalItems / pageSize),
    asOf: date,
  });
  return {
    async registryOptions() {
      return {
        tags: [
          { id: 'lua', label: 'Lua', groupName: 'Format' },
          { id: 'tools', label: 'Tools', groupName: 'Purpose' },
        ],
        gameBuilds: ['Steam:123', 'Other:fixture'],
      };
    },
    async registryList(query) {
      if (mode === 'offline')
        throw new Error(
          'Fixture registry unavailable. Installed mods remain usable.',
        );
      if (mode === 'slow')
        await new Promise((resolve) =>
          setTimeout(resolve, query.query === 'slow' ? 900 : 100),
        );
      let matches = mods.filter(
        (mod) =>
          `${mod.name} ${mod.summary} ${mod.owner?.displayName}`
            .toLowerCase()
            .includes(query.query.toLowerCase()) &&
          query.includeTags.every((tag) => mod.tags.includes(tag)) &&
          !query.excludeTags.some((tag) => mod.tags.includes(tag)) &&
          (query.modType === 'all' || query.modType === mod.modType) &&
          (query.maintenance === 'all' ||
            (query.maintenance === 'maintained') === mod.maintained) &&
          (!query.gameBuilds.length || query.gameBuilds.includes('Steam:123')),
      );
      if (query.sort === 'name')
        matches = [...matches].sort((a, b) => a.name.localeCompare(b.name));
      const page = Math.max(
        1,
        Math.min(query.page, Math.ceil(matches.length / query.pageSize)),
      );
      return {
        apiVersion: 1,
        items: matches.slice(
          (page - 1) * query.pageSize,
          page * query.pageSize,
        ),
        pagination: pagination(page, query.pageSize, matches.length),
      };
    },
    async registryDetail(id) {
      return fixtureDetail(id);
    },
    async registryRelease(id) {
      const parts = id.split('-');
      return fixtureRelease(Number(parts[0]), Number(parts[4]));
    },
    async registryHistory(id, page) {
      return {
        apiVersion: 1,
        items: [fixtureRelease(id), fixtureRelease(id, 1)].map(
          ({ metadata, ...release }) => ({
            ...release,
            metadataRevision: metadata.revision,
            testedGameBuild: metadata.testedGameBuild,
          }),
        ),
        pagination: pagination(page, 12, 2),
      };
    },
    async registryInstall() {
      if (mode === 'slow')
        await new Promise((resolve) => setTimeout(resolve, 900));
      throw new Error('Fixture install rejected. No game files changed.');
    },
  };
}
