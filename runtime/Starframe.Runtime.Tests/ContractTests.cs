using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Runtime.Versioning;
using System.Text.Json;
using Microsoft.VisualStudio.TestTools.UnitTesting;

namespace Starframe.Runtime.Tests;

[TestClass]
public sealed class ContractTests
{
    private static string Fixtures => Path.Combine(AppContext.BaseDirectory, "fixtures");
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
        Assert.AreEqual(".NETStandard,Version=v2.1", assembly.GetCustomAttribute<TargetFrameworkAttribute>()!.FrameworkName);
        Assert.AreEqual(typeof(ContractTests).Assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()!.InformationalVersion, assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()!.InformationalVersion);
    }

    [TestMethod]
    public void OversizedInputIsRejectedBeforeParsing()
    {
        Assert.ThrowsExactly<FormatException>(() => Contracts.Read(new byte[Contracts.MaxDocumentBytes + 1], "activation"));
    }
}
