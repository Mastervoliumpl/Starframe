using System;
using System.IO;
using System.Text.RegularExpressions;
using BepInEx.Configuration;
using Starframe.Runtime;

namespace Starframe.Bootstrap;

internal static class SettingsPersistence
{
    private static readonly object sync = new();
    public static ModSettings Open(string directory, string modId)
    {
        if (!Regex.IsMatch(modId, @"\A[a-z0-9][a-z0-9._-]{0,127}\z", RegexOptions.CultureInvariant))
            throw new ArgumentException("Invalid mod settings ID.", nameof(modId));
        string path = Path.Combine(directory, "mod." + modId + ".cfg");
        CheckPath(path);
        var config = new ConfigFile(path, false) { SaveOnConfigSet = false };
        return new ModSettings(key =>
        {
            // An absent entry has no saved value; the core retains its typed default.
            var entry = config.Bind<string>("Settings", key, "");
            return entry.Value.Length == 0 ? null : System.Text.Json.JsonSerializer.Deserialize<string>(entry.Value);
        }, (key, value) => { lock (sync) Save(path, key, value); });
    }

    private static void Save(string path, string key, string value)
    {
        CheckPath(path);
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        string temporary = path + "." + Guid.NewGuid().ToString("N") + ".tmp";
        try
        {
            if (File.Exists(path)) File.Copy(path, temporary);
            var staged = new ConfigFile(temporary, false) { SaveOnConfigSet = false };
            staged.Bind<string>("Settings", key, "").Value = System.Text.Json.JsonSerializer.Serialize(value);
            staged.Save();
            CheckPath(temporary);
            using (var file = new FileStream(temporary, FileMode.Open, FileAccess.Write, FileShare.None)) file.Flush(true);
            CheckPath(path);
            if (File.Exists(path)) File.Replace(temporary, path, null);
            else File.Move(temporary, path);
        }
        finally { if (File.Exists(temporary)) File.Delete(temporary); }
    }

    private static void CheckPath(string path)
    {
        if (File.Exists(path) && new FileInfo(path).Length > 2 * 1024 * 1024)
            throw new IOException("Mod settings exceed 2 MiB.");
        for (string? current = Path.GetFullPath(path); current != null; current = Path.GetDirectoryName(current))
            if ((File.Exists(current) || Directory.Exists(current)) && (File.GetAttributes(current) & FileAttributes.ReparsePoint) != 0)
                throw new IOException("Reparse points are not supported in mod settings paths.");
    }
}
