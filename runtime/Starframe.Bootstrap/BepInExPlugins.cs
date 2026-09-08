using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Text.RegularExpressions;
using BepInEx;
using BepInEx.Bootstrap;
using HarmonyLib;
using Starframe.Runtime;
using UnityEngine;
using Object = UnityEngine.Object;

namespace Starframe.Bootstrap;

internal sealed class BepInExPlugins(GameObject host)
{
    private readonly GameObject host = host;
    private readonly Dictionary<string, Entry> planned = new(StringComparer.Ordinal);
    private static Entry? starting;

    public IMod Prepare(string id, Type type, string assemblyPath)
    {
        if (!typeof(BaseUnityPlugin).IsAssignableFrom(type))
            throw new NotSupportedException("The entry must implement Starframe IMod or inherit BepInEx BaseUnityPlugin.");
        var metadata = MetadataHelper.GetMetadata(type);
        if (metadata == null || metadata.Version == null || string.IsNullOrWhiteSpace(metadata.Name)
            || !Regex.IsMatch(metadata.GUID ?? "", @"\A[a-z0-9][a-z0-9._-]{0,127}\z") || metadata.GUID != id)
            throw new NotSupportedException("Use the plugin's BepInPlugin GUID as the mod ID, with valid plugin name and version metadata.");
        var target = type.Assembly.GetReferencedAssemblies().FirstOrDefault(a => a.Name == "BepInEx")?.Version;
        var supported = typeof(BaseUnityPlugin).Assembly.GetName().Version!;
        if (target == null || target.Major != supported.Major || target.Minor > supported.Minor
            || (target.Minor == supported.Minor && target.Build > supported.Build))
            throw new NotSupportedException("The plugin requires a different BepInEx version.");
        var entry = new Entry(this, type, metadata, assemblyPath);
        planned.Add(id, entry);
        return entry;
    }

    private static void SetInfo(PluginInfo info, string property, object value) =>
        AccessTools.Property(typeof(PluginInfo), property).SetValue(info, value);

    private static Exception? CaptureStartup(Exception? __exception, object __instance)
    {
        if (__exception != null && starting?.Type.IsInstanceOfType(__instance) == true) starting.failure = __exception;
        return __exception;
    }

    private sealed class Entry(BepInExPlugins owner, Type type, BepInPlugin metadata, string assemblyPath) : IMod, IModShutdown
    {
        public Type Type => type;
        public Exception? failure;
        private BaseUnityPlugin? component;
        private PluginInfo? info;
        private readonly BepInDependency[] dependencies = MetadataHelper.GetDependencies(type).ToArray();
        private readonly BepInIncompatibility[] incompatible = MetadataHelper.GetAttributes<BepInIncompatibility>(type);

        public void Initialize(IModContext context)
        {
            if (Chainloader.PluginInfos.ContainsKey(metadata.GUID))
                throw new InvalidOperationException("A plugin with this GUID is already registered outside this activation: " + metadata.GUID);
            var processes = MetadataHelper.GetAttributes<BepInProcess>(type);
            if (processes.Length != 0 && processes.All(p => !string.Equals(Path.GetFileNameWithoutExtension(p.ProcessName), Paths.ProcessName, StringComparison.OrdinalIgnoreCase)))
                throw new NotSupportedException("The plugin's BepInProcess filter excludes this game.");
            foreach (var other in owner.planned.Values.Where(e => e != this))
                if (incompatible.Any(i => i.IncompatibilityGUID == other.Guid) || other.incompatible.Any(i => i.IncompatibilityGUID == metadata.GUID))
                    throw new InvalidOperationException("Incompatible enabled plugin: " + other.Guid);
            foreach (var other in Chainloader.PluginInfos.Values)
                if (incompatible.Any(i => i.IncompatibilityGUID == other.Metadata.GUID)
                    || (other.Incompatibilities?.Any(i => i.IncompatibilityGUID == metadata.GUID) ?? false))
                    throw new InvalidOperationException("Incompatible loaded plugin: " + other.Metadata.GUID);
            foreach (var dependency in dependencies)
            {
                bool required = (dependency.Flags & BepInDependency.DependencyFlags.HardDependency) != 0;
                if (Chainloader.PluginInfos.TryGetValue(dependency.DependencyGUID, out var loaded)
                    && loaded.Instance != null && loaded.Metadata.Version >= dependency.MinimumVersion) continue;
                if (required || owner.planned.ContainsKey(dependency.DependencyGUID))
                    throw new InvalidOperationException("Enable a compatible dependency before this plugin: " + dependency.DependencyGUID);
            }

            info = new PluginInfo();
            SetInfo(info, nameof(PluginInfo.Metadata), metadata);
            SetInfo(info, nameof(PluginInfo.Dependencies), dependencies);
            SetInfo(info, nameof(PluginInfo.Processes), processes);
            SetInfo(info, nameof(PluginInfo.Incompatibilities), incompatible);
            SetInfo(info, nameof(PluginInfo.Location), assemblyPath);
            Chainloader.PluginInfos.Add(metadata.GUID, info);
            var capture = new Harmony("starframe.plugin.startup");
            starting = this;
            try
            {
                // Unity logs callback exceptions instead of propagating them through AddComponent.
                foreach (string callback in new[] { "Awake", "OnEnable" })
                {
                    MethodInfo? method = null;
                    for (Type? declaring = type; declaring != null && method == null; declaring = declaring.BaseType)
                        method = declaring.GetMethod(callback, BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.DeclaredOnly, null, Type.EmptyTypes, null);
                    if (method != null) capture.Patch(method, finalizer: new HarmonyMethod(typeof(BepInExPlugins), nameof(CaptureStartup)));
                }
                component = owner.host.AddComponent(type) as BaseUnityPlugin;
                if (failure != null) throw new InvalidOperationException("BepInEx plugin startup failed: " + failure.Message, failure);
                if (component == null) throw new InvalidOperationException("Unity did not create the BepInEx plugin.");
                SetInfo(info, nameof(PluginInfo.Instance), component);
                BepInExSettings.Register(component.Config, context.Settings, context.Log);
                context.Log("BepInEx plugin started: " + metadata.Name + " " + metadata.Version);
            }
            finally { starting = null; capture.UnpatchSelf(); }
        }

        private string Guid => metadata.GUID;

        public void Shutdown()
        {
            if (component != null) Object.DestroyImmediate(component);
            component = null;
            if (info != null && Chainloader.PluginInfos.TryGetValue(metadata.GUID, out var current) && ReferenceEquals(current, info))
                Chainloader.PluginInfos.Remove(metadata.GUID);
            info = null;
        }
    }
}
