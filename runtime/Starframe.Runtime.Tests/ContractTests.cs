using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Runtime.Versioning;
using System.Text.Json;
using System.Text.Json.Nodes;
using Microsoft.VisualStudio.TestTools.UnitTesting;

namespace Starframe.Runtime.Tests;

[TestClass]
public sealed class ContractTests
{
    private static string Fixtures => Path.Combine(AppContext.BaseDirectory, "fixtures");

    [TestMethod]
    public void SharedActivationBoundaries()
    {
        using var cases = JsonDocument.Parse(File.ReadAllBytes(Path.Combine(Fixtures, "activation-boundaries.json")));
        foreach (var test in cases.RootElement.EnumerateArray())
        {
            string dimension = test.GetProperty("dimension").GetString()!;
            int count = test.GetProperty("count").GetInt32();
            int modCount = dimension switch { "inventory" => 0, "mods" => count, "files" => 1, _ => (count + 1023) / 1024 };
            int inventoryCount = dimension == "inventory" ? count : modCount;
            var inventory = new JsonArray();
            var mods = new JsonArray();
            for (int i = 0; i < inventoryCount; i++) inventory.Add(new JsonObject { ["modId"] = $"fixture.m{i}", ["name"] = "Fixture", ["version"] = "1" });
            for (int i = 0; i < modCount; i++)
            {
                int fileCount = dimension switch { "files" => count, "totalFiles" => Math.Min(count - i * 1024, 1024), _ => 1 };
                var files = new JsonArray();
                for (int f = 0; f < fileCount; f++) files.Add(new JsonObject { ["path"] = $"LJ/lua/f{f}.lua", ["sha256"] = string.Concat(Enumerable.Repeat("ab", 32)) });
                mods.Add(new JsonObject { ["modId"] = $"fixture.m{i}", ["root"] = $"mods/m{i}", ["source"] = new JsonObject { ["kind"] = "catalog", ["releaseId"] = $"fixture.m{i}.1" }, ["requires"] = new JsonArray(), ["entryAssembly"] = null, ["entryType"] = null, ["files"] = files });
            }
            var activation = new JsonObject { ["schemaVersion"] = 3, ["runtimeContractVersion"] = 1, ["integrationId"] = "starframe.bepinex", ["deploymentRevision"] = "1", ["installedMods"] = inventory, ["omittedDisabledMods"] = 0, ["mods"] = mods };
            bool accepted;
            try { using var parsed = Contracts.Read(JsonSerializer.SerializeToUtf8Bytes(activation), "activation"); accepted = true; }
            catch (FormatException) { accepted = false; }
            Assert.AreEqual(test.GetProperty("valid").GetBoolean(), accepted, test.ToString());
        }
    }
    public static IEnumerable<object[]> Cases()
    {
        using var manifest = JsonDocument.Parse(File.ReadAllBytes(Path.Combine(Fixtures, "cases.json")));
        return manifest.RootElement.EnumerateArray().Select(row => new object[] {
            row.GetProperty("file").GetString()!, row.GetProperty("kind").GetString()!, row.GetProperty("valid").GetBoolean()
        }).ToArray();
    }

    [TestMethod]
    [DynamicData(nameof(Cases))]
    public void SharedFixtures(string file, string kind, bool valid)
    {
        try
        {
            using var document = Contracts.Read(File.ReadAllBytes(Path.Combine(Fixtures, file)), kind);
            Assert.IsTrue(valid, file + " was accepted");
        }
        catch (Exception error) when (error is FormatException or JsonException or KeyNotFoundException or InvalidOperationException or ArgumentException)
        {
            Assert.IsFalse(valid, file + ": " + error.Message);
        }
    }

    [TestMethod]
    public void CanonicalContentAndTarget()
    {
        using var activation = Contracts.Read(File.ReadAllBytes(Path.Combine(Fixtures, "activation-local.json")), "activation");
        using var expected = JsonDocument.Parse(File.ReadAllBytes(Path.Combine(Fixtures, "canonical-inventory.json")));
        Assert.AreEqual(expected.RootElement.GetProperty("contentId").GetString(), Contracts.ContentId(activation.RootElement.GetProperty("mods")[0].GetProperty("files")));
        var assembly = typeof(Contracts).Assembly;
        Assert.AreEqual(".NETStandard,Version=v2.0", assembly.GetCustomAttribute<TargetFrameworkAttribute>()!.FrameworkName);
        Assert.AreEqual(typeof(ContractTests).Assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()!.InformationalVersion, assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()!.InformationalVersion);
    }

    [TestMethod]
    public void OversizedInputIsRejectedBeforeParsing()
    {
        Assert.ThrowsExactly<FormatException>(() => Contracts.Read(new byte[Contracts.MaxDocumentBytes + 1], "activation"));
    }
}
