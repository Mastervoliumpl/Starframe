using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Text.RegularExpressions;

namespace Starframe.Runtime;

public sealed class ModSettings
{
    private readonly Func<string, string?> read;
    private readonly Action<string, string> write;
    private readonly List<ModSetting> entries = new();
    public IReadOnlyList<ModSetting> Entries => entries.AsReadOnly();

    public ModSettings(Func<string, string?> read, Action<string, string> write)
    {
        this.read = read;
        this.write = write;
    }

    public ModSetting<T> Bind<T>(string key, string label, string description, T defaultValue, bool live = false)
    {
        if (!Regex.IsMatch(key, @"\A[a-z][a-z0-9_.-]{0,127}\z", RegexOptions.CultureInvariant))
            throw new ArgumentException("Use a stable lowercase setting key.", nameof(key));
        if (entries.Any(e => e.Key == key)) throw new ArgumentException("Setting key is already registered.", nameof(key));
        if (entries.Count >= 128) throw new InvalidOperationException("A mod can register at most 128 settings.");
        if (string.IsNullOrWhiteSpace(label) || label.Length > 160 || description.Length > 2000)
            throw new ArgumentException("Settings need a label of 1–160 characters and a description of at most 2000 characters.");
        var setting = new ModSetting<T>(key, label, description, defaultValue, live, read(key), value => write(key, value));
        entries.Add(setting);
        return setting;
    }
}

public abstract class ModSetting
{
    protected ModSetting(string key, string label, string description, bool live)
    { Key = key; Label = label; Description = description; Live = live; }
    public string Key { get; }
    public string Label { get; }
    public string Description { get; }
    public bool Live { get; }
    public string? Error { get; protected set; }
    public abstract Type ValueType { get; }
    public abstract string Text { get; }
    public abstract string DefaultText { get; }
    public abstract bool RestartRequired { get; }
    public abstract void SaveText(string text);
    public abstract void Reset();
}

public sealed class ModSetting<T> : ModSetting
{
    private readonly T defaultValue;
    private readonly Action<string> write;
    private T saved;
    public T Value { get; private set; }
    public override Type ValueType => typeof(T);
    public override string Text => Display(saved);
    public override string DefaultText => Display(defaultValue);
    public override bool RestartRequired => !EqualityComparer<T>.Default.Equals(saved, Value);

    internal ModSetting(string key, string label, string description, T defaultValue, bool live, string? stored, Action<string> write)
        : base(key, label, description, live)
    {
        if (typeof(T) != typeof(bool) && typeof(T) != typeof(int) && typeof(T) != typeof(float) && typeof(T) != typeof(string) && !typeof(T).IsEnum)
            throw new NotSupportedException("Settings support bool, int, float, string and named enum values.");
        Validate(defaultValue);
        this.defaultValue = defaultValue;
        this.write = write;
        saved = Value = defaultValue;
        if (stored == null) return;
        try { saved = Value = Parse(stored); }
        catch (Exception error) when (error is ArgumentException || error is FormatException || error is OverflowException)
        { Error = "Saved value is invalid. Using the default; the saved file has not been changed."; }
    }

    public override void SaveText(string text)
    {
        try
        {
            T next = Parse(text);
            write(Display(next));
            saved = next;
            if (Live) Value = next;
            Error = null;
        }
        catch (Exception error)
        {
            Error = error is ArgumentException || error is FormatException || error is OverflowException
                ? "Enter a valid " + typeof(T).Name + " value."
                : "Could not save this setting. " + error.Message;
            throw;
        }
    }

    public override void Reset() => SaveText(DefaultText);

    private static string Display(T value) => Convert.ToString(value, CultureInfo.InvariantCulture)!;

    private static T Parse(string text)
    {
        object parsed = typeof(T) == typeof(string) ? text
            : typeof(T).IsEnum ? (Enum.GetNames(typeof(T)).Contains(text) ? Enum.Parse(typeof(T), text, false) : throw new ArgumentException("Choose a named enum value."))
            : typeof(T) == typeof(float) ? float.Parse(text, NumberStyles.Float, CultureInfo.InvariantCulture)
            : Convert.ChangeType(text, typeof(T), CultureInfo.InvariantCulture);
        Validate((T)parsed);
        return (T)parsed;
    }

    private static void Validate(T value)
    {
        if (value == null || (value is string text && (text.Length > 4096 || text.Any(char.IsControl))))
            throw new ArgumentException("Text settings support up to 4096 characters without control characters.");
        if (value is float number && (float.IsNaN(number) || float.IsInfinity(number)))
            throw new ArgumentException("A numeric setting must be finite.");
        if (typeof(T).IsEnum && !Enum.IsDefined(typeof(T), value))
            throw new ArgumentException("Choose a named enum value.");
    }
}
