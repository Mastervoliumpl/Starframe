# Mod submissions and hosting

The owner raised this future direction on 8 September 2026, during milestone 0.5.0. It is recorded for later planning and does not change the current author-hosted download model or add server work to 0.5.0.

Explore a mod website with an author submission form that collects the metadata needed for installation and review. Mod pages could include a logo/avatar, screenshots, descriptions, download counts and likes/dislikes. Paradox Mods was the owner's product reference, not a selected service or integration.

Compare two delivery options before choosing infrastructure:

- Keep binaries at author-controlled URLs while Starframe hosts submissions, catalog metadata, images and community features.
- Accept binary uploads and operate mod storage and delivery as well.

A submission website does not require binary hosting. Hosting binaries would give Starframe more control over availability and download measurement, while adding storage, bandwidth, abuse handling, moderation and service operations. Define what a download count measures; a request alone does not establish a completed installation. Review telemetry, account and retention choices before collecting data.

For either option, define who supplies metadata, how maintainers approve exact releases, how authors update pages, and how reports and removals work. Package validation and tested runtime adapters remain necessary. A web form cannot establish that an arbitrary DLL is compatible with the game.

Related author-experience work to consider includes optional author manifests, metadata generation or prefilling where the answer is unambiguous, and clear handling of unknown package layouts. These are proposals, not implemented features or requirements for a published SDK. Representative author-mod compatibility remains a public-alpha gate in [issue #30](https://github.com/Mastervoliumpl/Starframe/issues/30).

Assign a version milestone and scoped issues after the owner selects a direction. No hosting service, telemetry collection or public upload flow is authorized by this note.
