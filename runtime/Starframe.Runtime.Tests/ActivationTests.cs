using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using System.Text.Json;
using Microsoft.VisualStudio.TestTools.UnitTesting;

namespace Starframe.Runtime.Tests;

[TestClass]
public sealed class ActivationTests
{
    [TestMethod]
    public void OrderedActivationFailurePropagationDisabledInventoryAndShutdown()
    {
        string root = Path.Combine(Path.GetTempPath(), "starframe-activation-" + Guid.NewGuid());
        Directory.CreateDirectory(root);
        try
        {
            byte[] dll = File.ReadAllBytes(Path.Combine(AppContext.BaseDirectory, "fixture-binaries/Starframe.FixtureMods.dll"));
            string hash = Convert.ToHexStringLower(SHA256.HashData(dll));
            var specs = new[] {
                ("first", "First", System.Array.Empty<string>()),
                ("second", "Second", new[]{"first"}),
                ("failure", "Failing", System.Array.Empty<string>()),
                ("dependent", "Second", new[]{"failure"})
            };
            foreach (var (id, _, _) in specs)
            {
                Directory.CreateDirectory(Path.Combine(root, id));
                File.WriteAllBytes(Path.Combine(root, id, "Starframe.FixtureMods.dll"), dll);
            }
            var manifest = JsonSerializer.SerializeToUtf8Bytes(new
            {
                schemaVersion = 2,
                runtimeContractVersion = 1,
                integrationId = "starframe.bepinex",
                deploymentRevision = "20",
                installedMods = specs.Select(s => s.Item1).Append("disabled").Select(id => new { modId = id, name = id, version = "1" }),
                mods = specs.Select(s => new
                {
                    modId = s.Item1,
                    source = new { kind = "catalog", releaseId = s.Item1 },
                    root = s.Item1,
                    entryAssembly = "Starframe.FixtureMods.dll",
                    entryType = "Starframe.FixtureMods." + s.Item2,
                    requires = s.Item3,
                    files = new[] { new { path = "Starframe.FixtureMods.dll", sha256 = hash } }
                })
            });
            var messages = new List<string>();
            using var session = new ActivationSession(messages.Add, new[] { "netstandard", "System.Runtime" });
            byte[] report = session.Activate(root, manifest);
            using var result = Contracts.Read(report, "report");
            CollectionAssert.AreEqual(new[] { "loaded", "loaded", "failed", "skipped_dependency" },
                result.RootElement.GetProperty("mods").EnumerateArray().Select(m => m.GetProperty("outcome").GetString()).ToArray());
            Assert.IsTrue(messages.IndexOf("first: first initialized") < messages.IndexOf("second: second initialized"));
            Assert.IsTrue(messages.Contains("failure: failed fixture cleaned up"));
            Assert.AreEqual(5, session.InstalledMods.Length);
            Assert.ThrowsExactly<InvalidOperationException>(() => session.Activate(root, manifest));
            string path = Path.Combine(root, "report.json");
            ActivationSession.WriteReport(path, report);
            ActivationSession.WriteReport(path, report);
            CollectionAssert.AreEqual(report, File.ReadAllBytes(path));
            session.Dispose();
            Assert.AreEqual("first: first stopped", messages.Last());
        }
        finally { Directory.Delete(root, true); }
    }

    [TestMethod]
    public void HashFailureAndContentOnlyNeverLoadAnAssembly()
    {
        string root = Path.Combine(Path.GetTempPath(), "starframe-content-" + Guid.NewGuid());
        Directory.CreateDirectory(Path.Combine(root, "example.core/1/lua"));
        try
        {
            string file = Path.Combine(root, "example.core/1/lua/example.lua");
            File.WriteAllText(file, "return 1");
            string manifest = File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "fixtures/activation-content.json"));
            foreach (bool valid in new[] { false, true })
            {
                string text = valid ? manifest.Replace(new string('0', 64), Convert.ToHexStringLower(SHA256.HashData(File.ReadAllBytes(file)))) : manifest;
                using var session = new ActivationSession(_ => { }, System.Array.Empty<string>());
                using var report = Contracts.Read(session.Activate(root, System.Text.Encoding.UTF8.GetBytes(text)), "report");
                Assert.AreEqual(valid ? "unsupported_content" : "invalid_payload", report.RootElement.GetProperty("mods")[0].GetProperty("errorCode").GetString());
            }
        }
        finally { Directory.Delete(root, true); }
    }
}
