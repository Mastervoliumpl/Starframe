using System;
using System.Globalization;
using System.Linq;
using BepInEx.Configuration;
using Starframe.Runtime;

namespace Starframe.Bootstrap;

internal static class BepInExSettings
{
    public static void Register(ConfigFile config, ModSettings settings, Action<string> log)
    {
        int index = 0;
        foreach (var entry in config.Select(pair => pair.Value).OrderBy(e => e.Definition.Section, StringComparer.Ordinal).ThenBy(e => e.Definition.Key, StringComparer.Ordinal))
        {
            if (index >= 128) { log("Only the first 128 BepInEx settings are shown. Other entries remain in the plugin configuration file."); break; }
            var type = entry.SettingType;
            if (type != typeof(bool) && type != typeof(int) && type != typeof(float) && type != typeof(string) && !type.IsEnum)
            { log("Setting remains in the plugin configuration file: " + entry.Definition); continue; }
            settings.Register(new Entry(config, entry, "bepinex." + (index++).ToString(CultureInfo.InvariantCulture)));
        }
    }

    private sealed class Entry(ConfigFile config, ConfigEntryBase entry, string key) : ModSetting(key,
        Limit(entry.Definition.Section + " / " + entry.Definition.Key, 160),
        Limit(entry.Description.Description, 2000), false)
    {
        private readonly string initial = entry.GetSerializedValue();
        public override bool RequiresMainThread => true;
        public override string ApplyDescription => "Changes are sent to the plugin. Restart to ensure they take effect.";
        public override Type ValueType => entry.SettingType;
        public override string Text => entry.GetSerializedValue();
        public override string DefaultText => TomlTypeConverter.ConvertToString(entry.DefaultValue, entry.SettingType);
        public override bool RestartRequired => Text != initial;

        public override void SaveText(string text)
        {
            try
            {
                if (text.Length > 4096 || text.Any(char.IsControl)) throw new ArgumentException("Use at most 4096 characters without control characters.");
                object value = TomlTypeConverter.ConvertToValue(text, entry.SettingType);
                if (value is float number && (float.IsNaN(number) || float.IsInfinity(number))) throw new ArgumentException("Use a finite number.");
                if (entry.SettingType.IsEnum && !Enum.IsDefined(entry.SettingType, value)) throw new ArgumentException("Choose a named enum value.");
                if (entry.Description.AcceptableValues != null && !entry.Description.AcceptableValues.IsValid(value))
                    throw new ArgumentException("The value is outside the plugin's allowed values.");
                entry.BoxedValue = value;
                config.Save();
                Error = null;
            }
            catch (Exception error) { Error = "Could not save this setting. " + error.Message; throw; }
        }

        public override void Reset() => SaveText(DefaultText);
    }

    private static string Limit(string value, int length) => value.Length <= length ? value : value.Substring(0, length);
}
