using System;
using System.IO;
using System.Linq;
using BepInEx;
using HarmonyLib;
using Starframe.Runtime;

namespace Starframe.Bootstrap;

[BepInPlugin("starframe.runtime", "Starframe", "0.2.0")]
[BepInProcess("Sanctuary.exe")]
public sealed class Plugin : BaseUnityPlugin
{
    private ActivationSession? session;
    private ModsMenu? menu;
    private static Plugin? active;
    private Harmony? patches;
    private void Awake()
    {
        try
        {
            gameObject.hideFlags = UnityEngine.HideFlags.HideAndDontSave;
            active = this;
            patches = new Harmony("starframe.runtime.lua");
            patches.Patch(AccessTools.Method(typeof(EM.Lua.FilesCache), "CreateFileCache"),
                postfix: new HarmonyMethod(typeof(Plugin), nameof(CacheReady)));
            if (EM.Lua.FilesCache.pathToFileContents != null) Activate();
        }
        catch (Exception error) { Logger.LogError("Starframe cache integration failed: " + error); }
    }
    private static void CacheReady() => active?.Activate();
    private void Activate()
    {
        if (session != null) return;
        try
        {
            string root = Path.Combine(Paths.GameRootPath, "Starframe");
            var overlays = new LuaOverlays(message => Logger.LogInfo(message));
            var plugins = new BepInExPlugins(gameObject);
            session = new ActivationSession(message => Logger.LogInfo(message),
                Directory.GetFiles(Paths.ManagedPath, "*.dll").Select(Path.GetFileNameWithoutExtension)!,
                id => SettingsPersistence.Open(Path.Combine(Paths.ConfigPath, "Starframe"), id), overlays.Apply, plugins.Prepare);
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
    private void OnDestroy() { menu?.Dispose(); session?.Dispose(); patches?.UnpatchSelf(); active = null; }
}
