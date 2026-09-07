using System;
using System.IO;
using System.Linq;
using BepInEx;
using Starframe.Runtime;

namespace Starframe.Bootstrap;

[BepInPlugin("starframe.runtime", "Starframe", "0.2.0")]
[BepInProcess("Sanctuary.exe")]
public sealed class Plugin : BaseUnityPlugin
{
    private ActivationSession? session;
    private void Awake()
    {
        try
        {
            gameObject.hideFlags = UnityEngine.HideFlags.HideAndDontSave;
            string root = Path.Combine(Paths.GameRootPath, "Starframe");
            session = new ActivationSession(message => Logger.LogInfo(message),
                Directory.GetFiles(Paths.ManagedPath, "*.dll").Select(Path.GetFileNameWithoutExtension)!);
            var report = session.Activate(root, ActivationSession.ReadManifest(Path.Combine(root, "activation.json")));
            ActivationSession.WriteReport(Path.Combine(root, "report.json"), report);
            Logger.LogInfo("Starframe activation report written for session " + session.SessionId);
        }
        catch (Exception error) { Logger.LogError("Starframe activation failed: " + error); }
    }
    private void OnDestroy() => session?.Dispose();
}
