using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace Starframe.Runtime;

public static class Contracts
{
    public const int MaxDocumentBytes = 1_048_576;
    public const int MaxMods = 256;
    public const int MaxFilesPerMod = 1024;

    public static JsonDocument Read(byte[] utf8, string kind)
    {
        Require(utf8.Length <= MaxDocumentBytes, "document size");
        var document = JsonDocument.Parse(utf8, new JsonDocumentOptions { MaxDepth = 32 });
        try
        {
            var root = document.RootElement;
            CheckDuplicates(root);
            string schema = root.GetProperty("schemaVersion").GetRawText();
            Require(kind == "activation" ? schema is "2" or "3" : schema == "1", "schema version");
            Require(root.GetProperty("runtimeContractVersion").GetRawText() == "1", "runtime contract version");
            Require(Text(root, "integrationId", 64) == "starframe.bepinex", "integration ID");
            switch (kind)
            {
                case "activation": Activation(root); break;
                case "capabilities": Capabilities(root); break;
                case "report": Report(root); break;
                default: throw new FormatException("Unknown document kind");
            }
            return document;
        }
        catch
        {
            document.Dispose();
            throw;
        }
    }

    private static void Activation(JsonElement root)
    {
        var fields = new List<string> { "schemaVersion", "runtimeContractVersion", "integrationId", "deploymentRevision", "installedMods", "mods" };
        int omitted = 0;
        if (root.GetProperty("schemaVersion").GetInt32() == 3)
        {
            fields.Add("omittedDisabledMods");
            Require(root.GetProperty("omittedDisabledMods").TryGetInt32(out omitted) && omitted >= 0, "omitted inventory count");
        }
        Fields(root, fields.ToArray());
        Decimal(root, "deploymentRevision");
        var installed = new HashSet<string>(StringComparer.Ordinal);
        foreach (var mod in Array(root, "installedMods", MaxMods))
        {
            Fields(mod, "modId", "name", "version");
            Require(installed.Add(Id(mod, "modId")), "duplicate inventory ID");
            Text(mod, "name", 256);
            Text(mod, "version", 128);
        }
        Require(omitted == 0 || installed.Count == MaxMods, "incomplete bounded inventory");
        var active = new HashSet<string>(StringComparer.Ordinal);
        var roots = new List<string>();
        int totalFiles = 0;
        foreach (var mod in Array(root, "mods", MaxMods))
        {
            Fields(mod, "modId", "source", "root", "entryAssembly", "entryType", "requires", "files");
            string id = Id(mod, "modId");
            Require(installed.Contains(id) && !active.Contains(id), "activation inventory mismatch");
            var dependencies = new HashSet<string>(StringComparer.Ordinal);
            foreach (var dependency in Array(mod, "requires", 256))
            {
                string required = String(dependency, 128);
                Require(active.Contains(required) && dependencies.Add(required), "dependency order");
            }
            active.Add(id);
            string directory = Path(Text(mod, "root", 240)).ToLowerInvariant();
            Require(!roots.Any(other => Overlaps(directory, other)), "overlapping roots");
            roots.Add(directory);
            bool content = mod.GetProperty("entryAssembly").ValueKind == JsonValueKind.Null;
            string? assembly = content ? null : Path(Text(mod, "entryAssembly", 240)).ToLowerInvariant();
            Require(content ? mod.GetProperty("entryType").ValueKind == JsonValueKind.Null : assembly!.EndsWith(".dll", StringComparison.Ordinal), "entry assembly");
            if (!content) Require(Matches(Text(mod, "entryType", 256), @"[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z_][A-Za-z0-9_]*)*"), "entry type");
            var paths = new HashSet<string>(StringComparer.Ordinal);
            foreach (var file in Array(mod, "files", MaxFilesPerMod))
            {
                Fields(file, "path", "sha256");
                string path = Path(Text(file, "path", 240)).ToLowerInvariant();
                Require(!paths.Any(other => Overlaps(path, other)) && paths.Add(path), "conflicting files");
                Require(Matches(Text(file, "sha256", 64), "[0-9a-f]{64}"), "file hash");
                Require(++totalFiles <= 8192, "total files");
            }
            Require(content ? paths.Count > 0 : paths.Contains(assembly!), "entry assembly missing from files");
            var source = mod.GetProperty("source");
            switch (Text(source, "kind", 16))
            {
                case "catalog": Fields(source, "kind", "releaseId"); Id(source, "releaseId"); break;
                case "local":
                    Fields(source, "kind", "contentId");
                    Require(Text(source, "contentId", 71) == ContentId(mod.GetProperty("files")), "local content ID");
                    break;
                default: throw new FormatException("Unsupported source kind");
            }
        }
    }

    public static string ContentId(JsonElement files)
    {
        var lines = files.EnumerateArray().Select(file =>
            file.GetProperty("path").GetString()!.ToLowerInvariant() + "\0" + file.GetProperty("sha256").GetString() + "\n");
        string canonical = "starframe-inventory-v1\n" + string.Concat(lines.OrderBy(line => line, StringComparer.Ordinal));
        using var sha = SHA256.Create();
        return "sha256:" + string.Concat(sha.ComputeHash(Encoding.UTF8.GetBytes(canonical)).Select(b => b.ToString("x2", CultureInfo.InvariantCulture)));
    }

    private static void Capabilities(JsonElement root)
    {
        Fields(root, "schemaVersion", "runtimeContractVersion", "integrationId", "capabilities");
        var caps = root.GetProperty("capabilities");
        Fields(caps, "managedLifecycle", "manualPriority", "contentOverlays", "settingsUi", "activationReport");
        foreach (var property in caps.EnumerateObject())
        {
            Fields(property.Value, "supported", "reason");
            var supported = property.Value.GetProperty("supported");
            Require(supported.ValueKind is JsonValueKind.True or JsonValueKind.False, "capability support");
            String(property.Value.GetProperty("reason"), 512, supported.GetBoolean());
        }
    }

    private static void Report(JsonElement root)
    {
        Fields(root, "schemaVersion", "runtimeContractVersion", "integrationId", "deploymentRevision", "gameSessionId", "processId", "processStartFileTime", "mods");
        Decimal(root, "deploymentRevision");
        Decimal(root, "processStartFileTime");
        string session = Text(root, "gameSessionId", 36);
        Require(Guid.TryParseExact(session, "D", out var guid) && guid != Guid.Empty && session == guid.ToString("D"), "session ID");
        Require(root.GetProperty("processId").TryGetUInt32(out uint pid) && pid > 0, "process ID");
        var ids = new HashSet<string>(StringComparer.Ordinal);
        foreach (var mod in Array(root, "mods", MaxMods))
        {
            Fields(mod, "modId", "outcome", "errorCode", "message");
            Require(ids.Add(Id(mod, "modId")), "duplicate report ID");
            string outcome = Text(mod, "outcome", 32);
            bool loaded = outcome == "loaded";
            Require(loaded || outcome is "failed" or "skipped_dependency", "outcome");
            string code = String(mod.GetProperty("errorCode"), 64, loaded);
            string message = String(mod.GetProperty("message"), 2048, loaded);
            Require(loaded ? code == "" && message == "" : Matches(code, "[a-z][a-z0-9_]*"), "failure details");
            Require(outcome != "skipped_dependency" || code == "dependency_failed", "dependency error code");
        }
    }

    private static void CheckDuplicates(JsonElement value)
    {
        if (value.ValueKind == JsonValueKind.Object)
        {
            var names = new HashSet<string>(StringComparer.Ordinal);
            foreach (var property in value.EnumerateObject())
            {
                Require(names.Add(property.Name), "duplicate JSON property");
                CheckDuplicates(property.Value);
            }
        }
        else if (value.ValueKind == JsonValueKind.Array)
            foreach (var element in value.EnumerateArray()) CheckDuplicates(element);
    }

    private static void Fields(JsonElement value, params string[] expected)
    {
        Require(value.ValueKind == JsonValueKind.Object, "expected object");
        var names = value.EnumerateObject().Select(p => p.Name).ToArray();
        Require(names.Length == expected.Length && expected.All(names.Contains), "missing or unknown field");
    }
    private static JsonElement.ArrayEnumerator Array(JsonElement value, string key, int max)
    {
        var array = value.GetProperty(key);
        Require(array.ValueKind == JsonValueKind.Array && array.GetArrayLength() <= max, "array limit");
        return array.EnumerateArray();
    }
    private static string Text(JsonElement value, string key, int max) => String(value.GetProperty(key), max);
    private static string String(JsonElement value, int max, bool empty = false)
    {
        Require(value.ValueKind == JsonValueKind.String, "expected string");
        string text = value.GetString()!;
        Require((empty || text.Length > 0) && Encoding.UTF8.GetByteCount(text) <= max && !text.Any(char.IsControl), "string limit or control character");
        return text;
    }
    private static string Id(JsonElement value, string key)
    {
        string id = Text(value, key, 128);
        Require(Matches(id, "[a-z0-9][a-z0-9._-]*"), "identifier");
        return id;
    }
    private static void Decimal(JsonElement value, string key) => Require(Matches(Text(value, key, 20), "0|[1-9][0-9]{0,19}"), "decimal string");
    private static string Path(string path)
    {
        foreach (string part in path.Split('/'))
        {
            Require(Matches(part, "[A-Za-z0-9_ .-]+") && part is not "." and not ".." && !part.EndsWith(".", StringComparison.Ordinal) && !part.EndsWith(" ", StringComparison.Ordinal), "relative path");
            string stem = part.Split('.')[0].TrimEnd(' ').ToUpperInvariant();
            Require(!Matches(stem, "CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9]"), "Windows device path");
        }
        return path;
    }
    private static bool Overlaps(string a, string b) => a == b || a.StartsWith(b + "/", StringComparison.Ordinal) || b.StartsWith(a + "/", StringComparison.Ordinal);
    private static bool Matches(string value, string pattern) => Regex.IsMatch(value, "\\A(?:" + pattern + ")\\z", RegexOptions.CultureInvariant);
    private static void Require(bool condition, string reason)
    {
        if (!condition) throw new FormatException("Invalid runtime contract: " + reason);
    }
}
