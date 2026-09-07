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
    private ModsMenu? menu;
    private void Awake()
    {
        try
        {
            gameObject.hideFlags = UnityEngine.HideFlags.HideAndDontSave;
            string root = Path.Combine(Paths.GameRootPath, "Starframe");
            session = new ActivationSession(message => Logger.LogInfo(message),
                Directory.GetFiles(Paths.ManagedPath, "*.dll").Select(Path.GetFileNameWithoutExtension)!,
                id => SettingsPersistence.Open(Path.Combine(Paths.ConfigPath, "Starframe"), id));
            var report = session.Activate(root, ActivationSession.ReadManifest(Path.Combine(root, "activation.json")));
            ActivationSession.WriteReport(Path.Combine(root, "report.json"), report);
            try { menu = new ModsMenu(this, session, report); }
            catch (Exception error) { Logger.LogError("Starframe mod menu is unavailable: " + error); }
            Logger.LogInfo("Starframe activation report written for session " + session.SessionId);
        }
        catch (Exception error) { Logger.LogError("Starframe activation failed: " + error); }
    }
    private void Update()
    {
        try { menu?.Tick(); }
        catch (Exception error)
        {
            Logger.LogError("Starframe mod menu is unavailable: " + error);
            menu?.Dispose();
            menu = null;
        }
    }
    private void OnDestroy() { menu?.Dispose(); session?.Dispose(); }
}
