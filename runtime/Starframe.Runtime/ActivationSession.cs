using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace Starframe.Runtime;

public sealed class ActivationSession : IDisposable
{
    private readonly Action<string> log;
    private readonly Dictionary<string, byte[]> assemblies = new(StringComparer.OrdinalIgnoreCase);
    private readonly Dictionary<string, Assembly> loaded = new(StringComparer.OrdinalIgnoreCase);
    private readonly HashSet<string> trusted;
    private readonly List<IModShutdown> shutdown = new();
    private bool started;
    public string SessionId { get; } = Guid.NewGuid().ToString("D");
    public JsonElement[] InstalledMods { get; private set; } = System.Array.Empty<JsonElement>();

    public ActivationSession(Action<string> log, IEnumerable<string> gameAssemblies)
    {
        this.log = log;
        trusted = new HashSet<string>(AppDomain.CurrentDomain.GetAssemblies().Select(a => a.GetName().Name!), StringComparer.OrdinalIgnoreCase);
        trusted.UnionWith(gameAssemblies);
    }

    public byte[] Activate(string root, byte[] manifest)
    {
        if (started) throw new InvalidOperationException("Activation requires a new game process.");
        started = true;
        using var document = Contracts.Read(manifest, "activation");
        var activation = document.RootElement;
        InstalledMods = activation.GetProperty("installedMods").EnumerateArray().Select(m => m.Clone()).ToArray();
        root = Path.GetFullPath(root);
        var errors = new Dictionary<string, string>();
        long total = 0;
        foreach (var mod in activation.GetProperty("mods").EnumerateArray())
        {
            string id = mod.GetProperty("modId").GetString()!;
            var own = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);
            try
            {
                foreach (var file in mod.GetProperty("files").EnumerateArray())
                {
                    string relative = file.GetProperty("path").GetString()!;
                    string path = Path.Combine(root, mod.GetProperty("root").GetString()!, relative);
                    CheckPath(path);
                    using var input = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read);
                    if (input.Length > 64 * 1024 * 1024 || (total += input.Length) > 256 * 1024 * 1024)
                        throw new IOException("Activation content exceeds the verified size limit.");
                    using var sha = SHA256.Create();
                    string digest = string.Concat(sha.ComputeHash(input).Select(b => b.ToString("x2", CultureInfo.InvariantCulture)));
                    if (digest != file.GetProperty("sha256").GetString()) throw new IOException("File hash mismatch: " + relative);
                    if (relative.EndsWith(".dll", StringComparison.OrdinalIgnoreCase))
                    {
                        if (input.Length > 16 * 1024 * 1024) throw new IOException("Managed assembly exceeds 16 MiB.");
                        input.Position = 0;
                        using var bytes = new MemoryStream();
                        input.CopyTo(bytes);
                        byte[] verified = bytes.ToArray();
                        string copiedHash = string.Concat(sha.ComputeHash(verified).Select(b => b.ToString("x2", CultureInfo.InvariantCulture)));
                        if (copiedHash != digest) throw new IOException("Assembly changed during verification.");
                        own.Add(Path.GetFileNameWithoutExtension(relative), verified);
                    }
                }
                foreach (var pair in own)
                {
                    if (trusted.Contains(pair.Key)) throw new IOException("Mod assembly conflicts with the game/runtime: " + pair.Key);
                    if (assemblies.TryGetValue(pair.Key, out var previous) && !previous.SequenceEqual(pair.Value))
                        throw new IOException("Conflicting managed assembly identity: " + pair.Key);
                }
                foreach (var pair in own) assemblies[pair.Key] = pair.Value;
            }
            catch (Exception error) { errors[id] = Message(error); }
        }
        AppDomain.CurrentDomain.AssemblyResolve += Resolve;
        var results = new List<object>();
        var successful = new HashSet<string>();
        foreach (var mod in activation.GetProperty("mods").EnumerateArray())
        {
            string id = mod.GetProperty("modId").GetString()!;
            string outcome = "loaded", code = "", message = "";
            IMod? instance = null;
            try
            {
                if (mod.GetProperty("requires").EnumerateArray().Any(d => !successful.Contains(d.GetString()!)))
                {
                    outcome = "skipped_dependency";
                    code = "dependency_failed";
                    message = "A required mod failed to activate.";
                }
                else if (errors.TryGetValue(id, out var invalid))
                {
                    outcome = "failed"; code = "invalid_payload"; message = invalid;
                }
                else if (mod.GetProperty("entryAssembly").ValueKind == JsonValueKind.Null)
                {
                    outcome = "failed"; code = "unsupported_content";
                    message = "Content metadata is supported; this runtime has no verified content activation adapter yet.";
                }
                else
                {
                    string name = Path.GetFileNameWithoutExtension(mod.GetProperty("entryAssembly").GetString()!);
                    var assembly = Load(name);
                    var type = assembly.GetType(mod.GetProperty("entryType").GetString()!, true)!;
                    if (type.IsAbstract || !typeof(IMod).IsAssignableFrom(type))
                        throw new NotSupportedException("Entry type must implement Starframe IMod. Conventional BepInEx plugins require a compatibility adapter.");
                    instance = (IMod)Activator.CreateInstance(type)!;
                    instance.Initialize(new ModContext(id, Path.Combine(root, mod.GetProperty("root").GetString()!), log));
                    if (instance is IModShutdown stop) shutdown.Add(stop);
                    successful.Add(id);
                }
            }
            catch (Exception error)
            {
                outcome = "failed"; code = "initialization_failed"; message = Message(error);
                if (instance is IModShutdown stop)
                    try { stop.Shutdown(); } catch (Exception cleanup) { log(id + ": cleanup failed: " + Message(cleanup)); }
            }
            results.Add(new { modId = id, outcome, errorCode = code, message });
            log(id + ": " + outcome + (message.Length == 0 ? "" : " - " + message));
        }
        using var process = Process.GetCurrentProcess();
        byte[] report = JsonSerializer.SerializeToUtf8Bytes(new
        {
            schemaVersion = 1,
            runtimeContractVersion = 1,
            integrationId = "starframe.bepinex",
            deploymentRevision = activation.GetProperty("deploymentRevision").GetString(),
            gameSessionId = SessionId,
            processId = (uint)process.Id,
            processStartFileTime = process.StartTime.ToUniversalTime().ToFileTimeUtc().ToString(CultureInfo.InvariantCulture),
            mods = results
        });
        using var checkedReport = Contracts.Read(report, "report");
        return report;
    }

    private Assembly? Resolve(object? sender, ResolveEventArgs args)
    {
        var name = new AssemblyName(args.Name);
        if (trusted.Contains(name.Name!)) return null;
        if (!assemblies.ContainsKey(name.Name!)) throw new FileNotFoundException("Unlisted managed dependency: " + name.Name);
        var result = Load(name.Name!);
        if (!AssemblyName.ReferenceMatchesDefinition(name, result.GetName())) throw new FileLoadException("Managed dependency identity mismatch.");
        return result;
    }

    private Assembly Load(string name)
    {
        if (loaded.TryGetValue(name, out var previous)) return previous;
        if (AppDomain.CurrentDomain.GetAssemblies().Any(a => string.Equals(a.GetName().Name, name, StringComparison.OrdinalIgnoreCase)))
            throw new FileLoadException("Assembly was already loaded outside this session: " + name);
        var assembly = Assembly.Load(assemblies[name]);
        if (!string.Equals(assembly.GetName().Name, name, StringComparison.OrdinalIgnoreCase))
            throw new FileLoadException("Assembly identity differs from its inventory filename.");
        foreach (var dependency in assembly.GetReferencedAssemblies())
            if (!trusted.Contains(dependency.Name!) && !assemblies.ContainsKey(dependency.Name!))
                throw new FileNotFoundException("Unlisted managed dependency: " + dependency.Name);
        loaded.Add(name, assembly);
        return assembly;
    }

    public static byte[] ReadManifest(string path)
    {
        CheckPath(path);
        using var file = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read);
        if (file.Length > Contracts.MaxDocumentBytes) throw new IOException("Activation document exceeds its limit.");
        using var bytes = new MemoryStream();
        file.CopyTo(bytes);
        if (bytes.Length > Contracts.MaxDocumentBytes) throw new IOException("Activation document grew beyond its limit.");
        return bytes.ToArray();
    }

    public static void WriteReport(string path, byte[] bytes)
    {
        using var report = Contracts.Read(bytes, "report");
        CheckPath(Path.GetDirectoryName(path)!);
        if (File.Exists(path)) CheckPath(path);
        string temporary = path + "." + Guid.NewGuid().ToString("N") + ".tmp";
        using (var file = new FileStream(temporary, FileMode.CreateNew, FileAccess.Write, FileShare.None))
        { file.Write(bytes, 0, bytes.Length); file.Flush(true); }
        if (File.Exists(path)) File.Replace(temporary, path, null);
        else File.Move(temporary, path);
    }

    private static void CheckPath(string path)
    {
        for (string? current = Path.GetFullPath(path); current != null; current = Path.GetDirectoryName(current))
            if ((File.GetAttributes(current) & FileAttributes.ReparsePoint) != 0)
                throw new IOException("Reparse points are not supported in activation paths.");
    }

    private static string Message(Exception error)
    {
        if (error is TargetInvocationException && error.InnerException != null) error = error.InnerException;
        string value = new string(error.Message.Select(c => char.IsControl(c) ? ' ' : c).ToArray()).Trim();
        if (value.Length == 0) value = error.GetType().Name;
        while (Encoding.UTF8.GetByteCount(value) > 2000) value = value.Substring(0, value.Length - 1);
        return value;
    }

    public void Dispose()
    {
        for (int i = shutdown.Count - 1; i >= 0; i--)
            try { shutdown[i].Shutdown(); } catch (Exception error) { log("Shutdown failed: " + Message(error)); }
        shutdown.Clear();
        AppDomain.CurrentDomain.AssemblyResolve -= Resolve;
    }

    private sealed class ModContext(string id, string contentRoot, Action<string> log) : IModContext
    {
        public string ModId => id;
        public string ContentRoot => contentRoot;
        public void Log(string message) => log(id + ": " + message);
    }
}
