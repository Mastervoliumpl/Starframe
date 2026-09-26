using System;

namespace Starframe.Bootstrap;

internal sealed class MenuLifecycle
{
    private object? owner;
    private object? source;
    private int escapeFrame = -1;

    public void Build(object nextOwner, object nextSource, Action build, Action cleanup)
    {
        if (ReferenceEquals(owner, nextOwner) && ReferenceEquals(source, nextSource)) return;
        owner = nextOwner;
        source = nextSource;
        cleanup();
        try { build(); }
        catch { cleanup(); throw; }
    }

    public bool HandledEscape(int frame) => escapeFrame == frame;

    public void Escape(int frame, Action back)
    {
        if (HandledEscape(frame)) return;
        escapeFrame = frame;
        back();
    }
}
