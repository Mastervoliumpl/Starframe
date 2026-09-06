namespace Starframe.Runtime;

// Internal contract experiment; no SDK package or compatibility guarantee is published.
public interface IMod
{
    void Initialize(IModContext context);
}

public interface IModContext
{
    string ModId { get; }
    void Log(string message);
}

public interface IModShutdown
{
    void Shutdown();
}
