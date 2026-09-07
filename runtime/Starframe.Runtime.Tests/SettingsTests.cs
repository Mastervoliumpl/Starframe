using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using Microsoft.VisualStudio.TestTools.UnitTesting;

namespace Starframe.Runtime.Tests;

[TestClass]
public sealed class SettingsTests
{
    private enum Mode { Calm, Fast }

    [TestMethod]
    public void LiveRestartResetAndStableKeysSurviveNewRegistry()
    {
        var disk = new Dictionary<string, string>();
        ModSettings Open() => new(key => disk.TryGetValue(key, out var value) ? value : null, (key, value) => disk[key] = value);
        var registry = Open();
        var live = registry.Bind("hints", "Show hints", "Show fixture hints.", true, live: true);
        var restart = registry.Bind("mode", "Mode", "Fixture mode.", Mode.Calm);
        live.SaveText("False");
        Assert.IsFalse(live.Value);
        Assert.IsFalse(live.RestartRequired);
        restart.SaveText("Fast");
        Assert.AreEqual(Mode.Calm, restart.Value);
        Assert.AreEqual("Fast", restart.Text);
        Assert.IsTrue(restart.RestartRequired);
        var next = Open().Bind("mode", "Renamed label", "Changed description.", Mode.Calm);
        Assert.AreEqual(Mode.Fast, next.Value);
        Assert.IsFalse(next.RestartRequired);
        restart.Reset();
        Assert.AreEqual("Calm", disk["mode"]);
        Assert.IsFalse(restart.RestartRequired);
        Assert.IsFalse(Open().Bind("hints", "Hints", "", true, true).Value);
    }

    [TestMethod]
    public void SupportedTypesRoundTripIndependentlyOfCulture()
    {
        var disk = new Dictionary<string, string>();
        var previous = CultureInfo.CurrentCulture;
        try
        {
            CultureInfo.CurrentCulture = CultureInfo.GetCultureInfo("da-DK");
            var settings = new ModSettings(_ => null, (key, value) => disk[key] = value);
            var number = settings.Bind("speed", "Speed", "", 1.5f, true);
            number.SaveText("2.25");
            Assert.AreEqual(2.25f, number.Value);
            Assert.AreEqual("2.25", disk["speed"]);
            var count = settings.Bind("count", "Count", "", 1, true);
            count.SaveText("-20");
            Assert.AreEqual(-20, count.Value);
            var text = settings.Bind("name", "Name", "", "default", true);
            text.SaveText("=; # [quotes] \" hello");
            Assert.AreEqual("=; # [quotes] \" hello", text.Value);
        }
        finally { CultureInfo.CurrentCulture = previous; }
    }

    [TestMethod]
    public void InvalidStoredValuesArePreservedUntilExplicitSave()
    {
        int writes = 0;
        var settings = new ModSettings(_ => "NaN", (_, _) => writes++);
        var entry = settings.Bind("speed", "Speed", "", 1f, true);
        Assert.AreEqual(1f, entry.Value);
        Assert.IsNotNull(entry.Error);
        Assert.AreEqual(0, writes);
        Assert.ThrowsExactly<ArgumentException>(() => entry.SaveText("Infinity"));
        Assert.AreEqual(0, writes);
        entry.Reset();
        Assert.AreEqual(1, writes);
        Assert.IsNull(entry.Error);
    }

    [TestMethod]
    public void FailedSaveDoesNotChangeEffectiveOrPendingValues()
    {
        var settings = new ModSettings(_ => null, (_, _) => throw new IOException("Disk is full."));
        var entry = settings.Bind("enabled", "Enabled", "", true, true);
        Assert.ThrowsExactly<IOException>(() => entry.SaveText("False"));
        Assert.IsTrue(entry.Value);
        Assert.AreEqual("True", entry.Text);
        Assert.IsNotNull(entry.Error);
    }

    [TestMethod]
    public void RejectUnsupportedTypesDuplicateKeysAndInvalidInput()
    {
        var settings = new ModSettings(_ => null, (_, _) => { });
        Assert.ThrowsExactly<NotSupportedException>(() => settings.Bind("date", "Date", "", DateTime.UtcNow));
        Assert.ThrowsExactly<ArgumentException>(() => settings.Bind("../key", "Name", "", true));
        var number = settings.Bind("count", "Count", "", 1);
        Assert.ThrowsExactly<ArgumentException>(() => settings.Bind("count", "Other label", "", 2));
        Assert.ThrowsExactly<OverflowException>(() => number.SaveText("2147483648"));
        var mode = settings.Bind("mode", "Mode", "", Mode.Calm);
        Assert.ThrowsExactly<ArgumentException>(() => mode.SaveText("42"));
        var text = settings.Bind("text", "Text", "", "");
        Assert.ThrowsExactly<ArgumentException>(() => text.SaveText("line\nbreak"));
        Assert.ThrowsExactly<ArgumentException>(() => text.SaveText(new string('a', 4097)));
    }
}
