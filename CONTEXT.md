# Starframe terminology

Starframe manages mods and collections for Sanctuary: Shattered Sun. Use these terms consistently in planning, code, and user-facing descriptions.

## Language

**Mod**:
A modification with a stable identity, independent of any particular release or local build.
_Avoid_: Using a filename or version number as the mod's identity.

**Release**:
A particular published version of a mod. Approval applies to an individual release, not automatically to every release of its mod.
_Avoid_: Using release and mod interchangeably.

**Artifact**:
The specific downloadable file for a release, such as a ZIP archive. One release can offer several artifacts for different installation layouts.
_Avoid_: Treating every download attached to a release as interchangeable.

**Catalog**:
The maintainer's list of approved releases, their download locations, and relevant compatibility information.
_Avoid_: Marketplace, mod hosting service.

**Local import**:
A mod supplied from the user's computer rather than obtained through the curated catalog. It remains a managed mod with a distinct origin.
_Avoid_: Unmanaged mod, approved release.

**Local build**:
A particular revision of a locally developed mod. Replacing it does not make it a new catalog release.

**Library**:
The mods and versions available locally through Starframe. Being in the library does not necessarily mean a mod is enabled or loaded by the game.

**Collection**:
A named mod setup that the user can edit, select, and share. Exact release pinning and settings inclusion are separate policy decisions.
_Avoid_: Switching between collection, profile, and playset for the same concept.

**Active collection**:
The collection selected as the user's intended setup for the next launch.

**Enabled**:
Included in the intended setup. A pending change to that setup does not mean the running game has applied it.

**Deployment**:
The mod files and loader configuration prepared in a particular game installation. It can differ from the active collection while changes are pending.
_Avoid_: Treating download completion as deployment completion.

**Game installation**:
A specific installed copy of Sanctuary: Shattered Sun, including its executable, content, and build identity.

**Game build**:
The identified revision of the game against which mod compatibility can be assessed.
_Avoid_: Using mod version or app version to mean game build.

**Loader**:
Software that allows the game to load mods. BepInEx and Remmy's custom Mod Loader have separate roles in the loading chain.
_Avoid_: Calling the desktop app itself the DLL loader.

**App update**:
A new release of Starframe itself, separate from a mod release, catalog change, or game update.
