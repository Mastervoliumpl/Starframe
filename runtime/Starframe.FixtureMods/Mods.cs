using System;
using Starframe.Runtime;

namespace Starframe.FixtureMods;

public sealed class First : IMod, IModShutdown
{
    private IModContext? context;
    public void Initialize(IModContext context)
    {
        this.context = context;
        var hints = context.Settings.Bind("hints", "Show hints", "Show fixture hints.", true, live: true);
        context.Settings.Bind("count", "Marker count", "Number of fixture markers.", 3);
        context.Settings.Bind("speed", "Marker speed", "Fixture marker movement speed.", 1.5f, live: true);
        context.Settings.Bind("title", "Marker label", "Text shown by the fixture.", "Starframe", live: true);
        context.Settings.Bind("mode", "Marker mode", "Fixture mode selected when the game starts.", FixtureMode.Calm);
        context.Log("first initialized");
    }
    public void Shutdown() => context?.Log("first stopped");
}
public sealed class Second : IMod
{
    public void Initialize(IModContext context) => context.Log("second initialized");
}
public sealed class Failing : IMod, IModShutdown
{
    private IModContext? context;
    public void Initialize(IModContext context)
    {
        this.context = context;
        context.Log("fixture failure requested");
        throw new InvalidOperationException("Expected fixture initialization failure.");
    }
    public void Shutdown() => context?.Log("failed fixture cleaned up");
}

public enum FixtureMode { Calm, Fast }
