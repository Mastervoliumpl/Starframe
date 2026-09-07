using System;
using Starframe.Runtime;

namespace Starframe.FixtureMods;

public sealed class First : IMod, IModShutdown
{
    private IModContext? context;
    public void Initialize(IModContext context) { this.context = context; context.Log("first initialized"); }
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
