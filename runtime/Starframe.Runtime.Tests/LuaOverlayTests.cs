using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Microsoft.VisualStudio.TestTools.UnitTesting;

namespace Starframe.Runtime.Tests;

[TestClass]
public class LuaOverlayTests
{
    [TestMethod]
    public void VerifiedLuaUsesActivationOrderAndRejectsMapsAiAndChangedBytes()
    {
        string root = Path.Combine(Path.GetTempPath(), "starframe-lua-" + Guid.NewGuid());
        try
        {
            foreach (bool reverse in new[] { false, true })
                foreach (string path in new[] { "LJ/lua/fixture.lua", "Maps/map.sanmap", "LJ/lua/ai/fixture.lua" })
                {
                    var specs = (reverse ? new[] { "b", "a" } : new[] { "a", "b" }).Select(id =>
                    {
                        byte[] bytes = Encoding.UTF8.GetBytes("return '" + id + "'");
                        string file = Path.Combine(root, id, path);
                        Directory.CreateDirectory(Path.GetDirectoryName(file)!);
                        File.WriteAllBytes(file, bytes);
                        return new
                        {
                            modId = id,
                            source = new { kind = "catalog", releaseId = id },
                            root = id,
                            entryAssembly = (string?)null,
                            entryType = (string?)null,
                            requires = System.Array.Empty<string>(),
                            files = new[] { new { path, sha256 = Convert.ToHexStringLower(SHA256.HashData(bytes)) } }
                        };
                    }).ToArray();
                    byte[] manifest = JsonSerializer.SerializeToUtf8Bytes(new
                    {
                        schemaVersion = 2,
                        runtimeContractVersion = 1,
                        integrationId = "starframe.bepinex",
                        deploymentRevision = "1",
                        installedMods = specs.Select(s => new { s.modId, name = s.modId, version = "1" }),
                        mods = specs
                    });
                    var cache = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);
                    var applied = new List<string>();
                    using var session = new ActivationSession(_ => { }, System.Array.Empty<string>(), applyLua: (id, files) =>
                    {
                        applied.Add(id);
                        foreach (var file in files) cache[file.Key] = file.Value;
                    });
                    using var report = Contracts.Read(session.Activate(root, manifest), "report");
                    bool supported = path == "LJ/lua/fixture.lua";
                    Assert.AreEqual(supported ? "loaded" : "failed", report.RootElement.GetProperty("mods")[1].GetProperty("outcome").GetString());
                    if (supported)
                    {
                        CollectionAssert.AreEqual(specs.Select(s => s.modId).ToArray(), applied);
                        Assert.AreEqual("return '" + specs.Last().modId + "'", Encoding.UTF8.GetString(cache[path.ToUpperInvariant()]));
                        File.WriteAllText(Path.Combine(root, specs[0].modId, path), "changed");
                        using var invalid = new ActivationSession(_ => { }, System.Array.Empty<string>(), applyLua: (_, _) => { });
                        using var failed = Contracts.Read(invalid.Activate(root, manifest), "report");
                        Assert.AreEqual("invalid_payload", failed.RootElement.GetProperty("mods")[0].GetProperty("errorCode").GetString());
                    }
                    else Assert.AreEqual(0, applied.Count);
                }
        }
        finally { if (Directory.Exists(root)) Directory.Delete(root, true); }
    }
}
