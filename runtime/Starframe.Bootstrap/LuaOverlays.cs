using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using EM.Lua;
using Unity.Collections;
using UnityEngine;

namespace Starframe.Bootstrap;

internal sealed class LuaOverlays
{
    private readonly Action<string> log;
    private readonly List<NativeArray<byte>> retired = new();
    public LuaOverlays(Action<string> log)
    {
        this.log = log;
        Application.quitting += ReleaseRetired;
    }

    public void Apply(string modId, IReadOnlyDictionary<string, byte[]> files)
    {
        if (FilesCache.pathToFileContents == null) throw new InvalidOperationException("The game's Lua cache is not ready.");
        if (EM.GameUtils.DebugManager.data.miscelenous.enableFileWatcher)
            throw new NotSupportedException("Disable the game's development file watcher before using Lua overlays.");
        string root = Path.GetFullPath(Path.Combine(Application.dataPath, ".."));
        var allocated = new Dictionary<string, NativeArray<byte>>(StringComparer.OrdinalIgnoreCase);
        try
        {
            foreach (var file in files)
            {
                string path = Path.GetFullPath(Path.Combine(root, file.Key));
                if (!Directory.Exists(Path.GetDirectoryName(path)))
                    throw new NotSupportedException("Lua overlays require an existing game directory: " + file.Key);
                allocated.Add(path, new NativeArray<byte>(file.Value, Allocator.Persistent));
            }
            var next = new Dictionary<string, NativeArray<byte>>(FilesCache.pathToFileContents, StringComparer.OrdinalIgnoreCase);
            var replaced = new List<NativeArray<byte>>();
            foreach (var file in allocated)
            {
                if (next.TryGetValue(file.Key, out var previous)) replaced.Add(previous);
                next[file.Key] = file.Value;
            }
            retired.AddRange(replaced);
            FilesCache.pathToFileContents = next;
        }
        catch
        {
            foreach (var array in allocated.Values) array.Dispose();
            throw;
        }
        foreach (var file in files)
        {
            if (!FilesCache.TryGetFileContent(file.Key, out var actual) || !actual.ToArray().SequenceEqual(file.Value))
                throw new IOException("Game Lua lookup differs from the applied overlay: " + file.Key);
            using var sha = SHA256.Create();
            string digest = BitConverter.ToString(sha.ComputeHash(file.Value)).Replace("-", "").ToLowerInvariant();
            log("Lua overlay winner " + file.Key + ": " + modId + " sha256=" + digest);
        }
    }

    private void ReleaseRetired()
    {
        // Earlier mods can retain cache arrays; keep replaced bytes alive until the game exits.
        foreach (var array in retired) if (array.IsCreated) array.Dispose();
        retired.Clear();
        Application.quitting -= ReleaseRetired;
    }
}
