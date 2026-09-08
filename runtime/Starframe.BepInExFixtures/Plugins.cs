using System;
using BepInEx;
using BepInEx.Bootstrap;
using BepInEx.Configuration;

namespace Starframe.BepInExFixtures;

[BepInPlugin("fixture.bep.good", "BepInEx fixture", "1.0.0")]
public sealed class Good : BaseUnityPlugin
{
    private void Awake()
    {
        if (!ReferenceEquals(Chainloader.PluginInfos[Info.Metadata.GUID], Info) || string.IsNullOrEmpty(Info.Location))
            throw new InvalidOperationException("Plugin registry context is missing.");
        Config.Bind("Fixture", "Enabled", true, "Fixture switch.");
        Config.Bind("Fixture", "Count", 3, new ConfigDescription("Fixture count.", new AcceptableValueRange<int>(0, 50)));
        Config.Bind("Fixture", "Label", "Starframe", "Fixture text.");
        Logger.LogInfo("BEP_FIXTURE_STARTED count=" + Config.Bind("Fixture", "Count", 3).Value);
    }
    private void OnDestroy() => Logger.LogInfo("BEP_FIXTURE_STOPPED");
}

[BepInPlugin("fixture.bep.dependent", "BepInEx dependent", "1.0.0")]
[BepInDependency("fixture.bep.good", "1.0.0")]
public sealed class Dependent : BaseUnityPlugin
{
    private void Awake()
    {
        if (Chainloader.PluginInfos["fixture.bep.good"].Instance == null) throw new InvalidOperationException("Dependency did not start first.");
        Logger.LogInfo("BEP_DEPENDENT_STARTED");
    }
}

[BepInPlugin("fixture.bep.failure", "BepInEx failure", "1.0.0")]
public sealed class Failing : BaseUnityPlugin
{
    private void Awake() => throw new InvalidOperationException("BEP_EXPECTED_AWAKE_FAILURE");
}

[BepInPlugin("fixture.bep.missing", "BepInEx missing dependency", "1.0.0")]
[BepInDependency("fixture.bep.absent")]
public sealed class Missing : BaseUnityPlugin
{
    private void Awake() => throw new InvalidOperationException("MISSING_DEPENDENCY_MUST_NOT_START");
}

[BepInPlugin("fixture.bep.filtered", "BepInEx process filter", "1.0.0")]
[BepInProcess("AnotherGame.exe")]
public sealed class Filtered : BaseUnityPlugin
{
    private void Awake() => throw new InvalidOperationException("FILTERED_PLUGIN_MUST_NOT_START");
}

[BepInPlugin("fixture.bep.incompatible", "BepInEx incompatibility", "1.0.0")]
[BepInIncompatibility("fixture.bep.good")]
public sealed class Incompatible : BaseUnityPlugin
{
    private void Awake() => throw new InvalidOperationException("INCOMPATIBLE_PLUGIN_MUST_NOT_START");
}
