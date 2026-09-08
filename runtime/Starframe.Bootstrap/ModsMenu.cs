using System;
using System.Collections;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
using System.Threading.Tasks;
using EM.UI;
using HarmonyLib;
using Michsky.UI.Beam;
using Starframe.Runtime;
using TMPro;
using UnityEngine;
using UnityEngine.Events;
using UnityEngine.EventSystems;
using UnityEngine.UI;
using Object = UnityEngine.Object;

namespace Starframe.Bootstrap;

internal sealed class ModsMenu : IDisposable
{
    private readonly MonoBehaviour host;
    private readonly ActivationSession session;
    private readonly Dictionary<string, JsonElement> outcomes;
    private readonly Harmony harmony = new("starframe.mods.menu");
    private readonly List<GameObject> focus = new();
    private readonly List<Action<bool>> settingControls = new();
    private static ModsMenu? current;
    private MainMenuInterface? main;
    private GameObject? page;
    private PanelButton? entry;
    private Transform list = null!;
    private GameObject header = null!, body = null!, button = null!, toggle = null!, input = null!;
    private ButtonManager back = null!, reset = null!;
    private Sprite? icon;
    private string? selected;
    private int saves;
    private float listScroll = 1;

    public ModsMenu(MonoBehaviour host, ActivationSession session, byte[] report)
    {
        this.host = host;
        this.session = session;
        using var document = Contracts.Read(report, "report");
        outcomes = document.RootElement.GetProperty("mods").EnumerateArray().ToDictionary(m => m.GetProperty("modId").GetString()!, m => m.Clone());
        current = this;
        harmony.Patch(AccessTools.Method(typeof(InterfaceManager), nameof(InterfaceManager.TransitionTo)),
            prefix: new HarmonyMethod(typeof(ModsMenu), nameof(HideForTransition)));
    }

    private static void HideForTransition() { if (current?.page != null) current.page.SetActive(false); }

    public void Tick()
    {
        var found = MainMenuInterface.Instance;
        if (found != null && found != main)
        {
            main = found;
            Build(found);
        }
        if (page == null || !page.activeSelf) return;
        if (Input.GetKeyDown(KeyCode.Escape)) { Back(); return; }
        if (Input.GetKeyDown(KeyCode.Tab))
        {
            var available = focus.Where(g => g != null && g.activeInHierarchy && (g.GetComponent<Selectable>()?.IsInteractable() ?? true)).ToList();
            if (available.Count == 0) return;
            int index = available.IndexOf(EventSystem.current.currentSelectedGameObject);
            int step = Input.GetKey(KeyCode.LeftShift) || Input.GetKey(KeyCode.RightShift) ? -1 : 1;
            Select(available[(index + step + available.Count) % available.Count]);
        }
    }

    private void Build(MainMenuInterface menu)
    {
        if (page != null) Object.Destroy(page);
        if (entry != null) Object.Destroy(entry.gameObject);
        var original = menu.transform.parent.Find("SettingsInterface");
        page = Object.Instantiate(original.gameObject, original.parent);
        page.name = "StarframeMods";
        page.SetActive(false);
        Object.DestroyImmediate(page.GetComponent<SanctuaryUI.SettingsInterface>());
        var graphics = page.transform.Find("Content/Panels/Graphics");
        list = graphics.Find("Content/List/Layout Group");
        var layout = list.GetComponent<VerticalLayoutGroup>();
        layout.childControlHeight = true;
        layout.childForceExpandHeight = false;
        var templates = new GameObject("Templates");
        templates.transform.SetParent(page.transform, false);
        templates.SetActive(false);
        header = Keep(list.Find("Display Header"), templates.transform);
        body = Keep(page.transform.Find("Content/Description Area/Description"), templates.transform);
        button = Keep(list.Find("ApplyButton"), templates.transform);
        input = Keep(list.Find("UI Scale"), templates.transform);
        toggle = Keep(page.transform.Find("Content/Panels/Controls/Content/List/Layout Group/EdgePanToggle"), templates.transform);
        foreach (Transform panel in page.transform.Find("Content/Panels"))
            if (panel.name is "General" or "Audio" or "Controls") panel.gameObject.SetActive(false);
        var category = page.transform.Find("Content/Categories/Graphics").GetComponent<PanelButton>();
        foreach (Transform tab in category.transform.parent) if (tab != category.transform) tab.gameObject.SetActive(false);
        icon ??= LoadIcon();
        Configure(category, "Mods");
        category.useSeperator = false;
        category.buttonIcon = category.selectedIcon = icon;
        category.UpdateUI();
        var panels = page.GetComponent<PanelManager>();
        panels.panels = new List<PanelManager.PanelItem> { new() { panelName = "Mods", panelObject = graphics.GetComponent<Animator>(), panelButton = category } };
        panels.currentPanelIndex = 0;
        page.transform.Find("Content/Description Area").gameObject.SetActive(false);
        foreach (string path in new[] { "Content/Panels", "Content/Buttons" })
        {
            var rect = (RectTransform)page.transform.Find(path);
            rect.sizeDelta = new Vector2(-70, rect.sizeDelta.y);
            rect.anchoredPosition = new Vector2(0, rect.anchoredPosition.y);
        }
        back = page.transform.Find("Content/Buttons/BackButton").GetComponent<ButtonManager>();
        Configure(back, "Back", Back);
        reset = page.transform.Find("Content/Buttons/Reset Settings Button").GetComponent<ButtonManager>();
        Configure(reset, "Reset this mod", Reset);
        var settings = menu.transform.Find("Left Sidebar/Content/Button List/Settings");
        entry = Object.Instantiate(settings.gameObject, settings.parent).GetComponent<PanelButton>();
        entry.name = "StarframeModsButton";
        entry.transform.SetSiblingIndex(settings.GetSiblingIndex() + 1);
        Configure(entry, "Mods");
        entry.buttonIcon = entry.selectedIcon = icon;
        entry.onClick.AddListener(Open);
        entry.UpdateUI();
        entry.AddUINavigation();
        entry.GetComponentInParent<PanelButtonDimmer>()?.FetchButtons();
        Clear();
    }

    private static GameObject Keep(Transform source, Transform parent)
    {
        source.gameObject.SetActive(false);
        source.SetParent(parent, false);
        foreach (var localized in source.GetComponentsInChildren<LocalizedObject>(true)) Object.DestroyImmediate(localized);
        foreach (var description in source.GetComponentsInChildren<SettingsDescription>(true)) Object.DestroyImmediate(description);
        foreach (var text in source.GetComponentsInChildren<TMP_Text>(true)) text.richText = false;
        return source.gameObject;
    }

    private static void Configure(PanelButton control, string label)
    {
        control.useLocalization = false;
        control.useCustomText = false;
        control.buttonText = label;
        foreach (var text in control.GetComponentsInChildren<TMP_Text>(true)) { text.text = label; text.richText = false; }
        control.isInteractable = true;
        control.isSelected = false;
        control.useUINavigation = true;
        control.onClick = new UnityEvent();
        control.onSelect = new UnityEvent();
        control.onHover = new UnityEvent();
        control.onLeave = new UnityEvent();
    }

    private static void Configure(ButtonManager control, string label, Action action)
    {
        control.useLocalization = false;
        control.isInteractable = true;
        control.useUINavigation = true;
        control.onClick = new UnityEvent();
        control.onClick.AddListener(() => action());
        var native = control.GetComponent<Button>();
        if (native != null) native.onClick = new Button.ButtonClickedEvent();
        control.SetText(label);
    }

    private void Open()
    {
        InterfaceManager.Instance.TransitionTo(InterfaceManager.Window.Background);
        page!.SetActive(true);
        ShowList();
    }

    private void Back()
    {
        if (selected != null) { ShowList(); return; }
        page!.SetActive(false);
        InterfaceManager.Instance.TransitionTo(InterfaceManager.Window.Main);
        if (entry != null) Select(entry.gameObject);
    }

    private void Clear()
    {
        foreach (Transform child in list) { child.gameObject.SetActive(false); Object.Destroy(child.gameObject); }
        focus.Clear();
        settingControls.Clear();
    }

    private string Status(string id) => !outcomes.TryGetValue(id, out var outcome) ? "Disabled"
        : "Enabled · " + (outcome.GetProperty("outcome").GetString() == "loaded" ? "Loaded" : "Failed to load");

    private void ShowList()
    {
        string? previous = selected;
        selected = null;
        GameObject? restore = null;
        Clear();
        reset.gameObject.SetActive(false);
        Text("Installed mods", heading: true);
        Text("This game session. Change enabled mods in Starframe, then restart the game.");
        if (session.OmittedDisabledMods > 0) Text(session.OmittedDisabledMods + " more disabled mods are in your desktop library. Their saved settings are retained.");
        if (session.InstalledMods.Length == 0) Text("No installed mods in this session.");
        foreach (var mod in session.InstalledMods)
        {
            string id = mod.GetProperty("modId").GetString()!;
            var row = AddButton(mod.GetProperty("name").GetString() + " · " + mod.GetProperty("version").GetString() + " · " + Status(id), () => ShowMod(id));
            if (id == previous) restore = row.gameObject;
        }
        FinishPage(restore, previous == null ? 1 : listScroll);
    }

    private void ShowMod(string id)
    {
        if (selected == null) listScroll = list.GetComponentInParent<ScrollRect>().verticalNormalizedPosition;
        selected = id;
        Clear();
        var mod = session.InstalledMods.First(m => m.GetProperty("modId").GetString() == id);
        Text(mod.GetProperty("name").GetString() + " · " + Status(id), heading: true);
        bool loaded = session.Settings.TryGetValue(id, out var settings);
        reset.gameObject.SetActive(loaded && settings!.Entries.Count > 0);
        if (!outcomes.ContainsKey(id)) Text("Enable this mod in Starframe and restart the game to register its settings.");
        else if (!loaded) Text(outcomes[id].GetProperty("message").GetString()!);
        else if (settings!.Entries.Count == 0) Text("This mod has not registered any settings.");
        else foreach (var setting in settings.Entries) AddSetting(setting);
        FinishPage();
    }

    private void FinishPage(GameObject? restore = null, float scroll = 1)
    {
        focus.Add(back.gameObject);
        if (reset.gameObject.activeSelf) focus.Add(reset.gameObject);
        Canvas.ForceUpdateCanvases();
        list.GetComponentInParent<ScrollRect>().verticalNormalizedPosition = scroll;
        reset.Interactable(saves == 0);
        Select(restore != null ? restore : focus[0]);
    }

    private void Select(GameObject target)
    {
        EventSystem.current.SetSelectedGameObject(target);
        if (page == null || !target.transform.IsChildOf(list)) return;
        Canvas.ForceUpdateCanvases();
        var scroll = list.GetComponentInParent<ScrollRect>();
        var viewport = scroll.viewport != null ? scroll.viewport : (RectTransform)scroll.transform;
        var bounds = RectTransformUtility.CalculateRelativeRectTransformBounds(viewport, target.transform);
        float delta = bounds.min.y < viewport.rect.yMin ? viewport.rect.yMin - bounds.min.y
            : bounds.max.y > viewport.rect.yMax ? viewport.rect.yMax - bounds.max.y : 0;
        scroll.content.anchoredPosition += new Vector2(0, delta);
    }

    private TMP_Text Text(string value, bool heading = false)
    {
        var row = Object.Instantiate(heading ? header : body, list);
        var text = row.GetComponent<TMP_Text>();
        text.text = value;
        text.richText = false;
        text.alignment = TextAlignmentOptions.TopLeft;
        text.textWrappingMode = TextWrappingModes.Normal;
        text.overflowMode = TextOverflowModes.Overflow;
        var fit = row.AddComponent<ContentSizeFitter>();
        fit.verticalFit = ContentSizeFitter.FitMode.PreferredSize;
        row.SetActive(true);
        return text;
    }

    private ButtonManager AddButton(string label, Action action)
    {
        var row = Object.Instantiate(button, list);
        var control = row.GetComponent<ButtonManager>();
        Configure(control, label, action);
        control.autoFitContent = false;
        var fit = row.GetComponent<ContentSizeFitter>();
        fit.horizontalFit = ContentSizeFitter.FitMode.Unconstrained;
        fit.verticalFit = ContentSizeFitter.FitMode.Unconstrained;
        foreach (var text in row.GetComponentsInChildren<TMP_Text>(true))
        {
            text.textWrappingMode = TextWrappingModes.Normal;
            text.overflowMode = TextOverflowModes.Overflow;
        }
        var sizing = row.AddComponent<SettingsButtonSize>();
        sizing.Label = control.normalTextObj;
        sizing.Layout = row.AddComponent<LayoutElement>();
        row.SetActive(true);
        focus.Add(row);
        return control;
    }

    private void AddSetting(ModSetting setting)
    {
        TMP_Text? status = null;
        Action<bool> enable = _ => { };
        Action refresh = () => { };
        Action<string> save = value => host.StartCoroutine(Save(setting, value, status!, enable, refresh));
        if (setting.ValueType == typeof(bool))
        {
            var row = Object.Instantiate(toggle, list);
            row.transform.Find("Text").GetComponent<TMP_Text>().text = setting.Label;
            var control = row.transform.Find("Switch").GetComponent<SwitchManager>();
            control.saveValue = false;
            control.invokeOnEnable = false;
            control.useUINavigation = true;
            control.isOn = bool.Parse(setting.Text);
            control.onEvents = new UnityEvent();
            control.offEvents = new UnityEvent();
            control.onValueChanged = new SwitchManager.SwitchEvent();
            control.onValueChanged.AddListener(value => save(value.ToString()));
            enable = value => { if (control != null) control.isInteractable = value; };
            refresh = () => { if (control != null) { control.isOn = bool.Parse(setting.Text); control.UpdateUI(); } };
            FitRow(row, (RectTransform)control.transform);
            row.SetActive(true);
            focus.Add(control.gameObject);
        }
        else if (setting.ValueType.IsEnum)
        {
            var choices = Enum.GetNames(setting.ValueType);
            ButtonManager? control = null;
            control = AddButton(setting.Label + ": " + setting.Text, () =>
            {
                string next = choices[(Array.IndexOf(choices, setting.Text) + 1) % choices.Length];
                save(next);
            });
            enable = value => { if (control != null) control.Interactable(value); };
            refresh = () => { if (control != null) control.SetText(setting.Label + ": " + setting.Text); };
            control.gameObject.name = setting.Key;
        }
        else
        {
            var row = Object.Instantiate(input, list);
            Object.DestroyImmediate(row.GetComponent<SliderInputHandler>());
            row.transform.Find("Text").GetComponent<TMP_Text>().text = setting.Label;
            var slider = row.transform.Find("Slider");
            Object.DestroyImmediate(slider.GetComponent<SliderManager>());
            Object.DestroyImmediate(slider.GetComponent<Slider>());
            foreach (Transform child in slider) if (child.name != "Text Input" && child.name != "Static") child.gameObject.SetActive(false);
            var field = slider.GetComponentInChildren<TMP_InputField>(true);
            Object.DestroyImmediate(field.GetComponent<SliderInput>());
            var rect = (RectTransform)field.transform;
            rect.anchorMin = Vector2.zero; rect.anchorMax = Vector2.one;
            rect.offsetMin = rect.offsetMax = Vector2.zero;
            field.transform.SetAsLastSibling();
            field.contentType = TMP_InputField.ContentType.Standard;
            field.lineType = TMP_InputField.LineType.SingleLine;
            field.characterLimit = setting.ValueType == typeof(string) ? 4096 : 32;
            field.onValueChanged = new TMP_InputField.OnChangeEvent();
            field.onEndEdit = new TMP_InputField.SubmitEvent();
            field.SetTextWithoutNotify(setting.Text);
            field.onEndEdit.AddListener(value => { if (value != setting.Text || setting.Error != null) save(value); });
            enable = value => { if (field != null) field.interactable = value; };
            refresh = () => { if (field != null && setting.Error == null) field.SetTextWithoutNotify(setting.Text); };
            FitRow(row, (RectTransform)slider);
            row.SetActive(true);
            focus.Add(field.gameObject);
        }
        settingControls.Add(enable);
        Text(setting.Description + " Default: " + setting.DefaultText + ". " + (setting.Live ? "Applies while the game is running." : "Takes effect after restart."));
        status = Text(setting.Error ?? (setting.RestartRequired ? "Restart required" : ""));
        status.gameObject.name = "Status-" + setting.Key;
    }

    private static void FitRow(GameObject row, RectTransform control)
    {
        var label = row.transform.Find("Text").GetComponent<TMP_Text>();
        label.textWrappingMode = TextWrappingModes.Normal;
        label.overflowMode = TextOverflowModes.Overflow;
        label.rectTransform.anchorMin = Vector2.zero;
        label.rectTransform.anchorMax = Vector2.one;
        label.rectTransform.offsetMin = new Vector2(20, 8);
        label.rectTransform.offsetMax = new Vector2(-control.rect.width - 40, -8);
        var sizing = row.AddComponent<SettingsRowSize>();
        sizing.Label = label;
        sizing.Control = control;
        sizing.Layout = row.AddComponent<LayoutElement>();
        sizing.Minimum = ((RectTransform)row.transform).rect.height;
        foreach (var text in control.GetComponentsInChildren<TMP_Text>(true))
            if (text.GetComponentInParent<TMP_InputField>() == null) text.overflowMode = TextOverflowModes.Overflow;
    }

    private IEnumerator Save(ModSetting setting, string value, TMP_Text status, Action<bool> enable, Action refresh)
    {
        saves++;
        enable(false);
        reset.Interactable(false);
        if (status != null) status.text = "Saving…";
        var task = Task.Run(() => setting.SaveText(value));
        while (!task.IsCompleted) yield return null;
        if (task.IsFaulted) _ = task.Exception;
        saves--;
        enable(true);
        refresh();
        if (reset != null) reset.Interactable(saves == 0);
        if (status != null) status.text = task.IsFaulted ? setting.Error : setting.RestartRequired ? "Saved · Restart required" : "Saved";
    }

    private void Reset()
    {
        if (saves != 0 || selected == null || !session.Settings.TryGetValue(selected, out var settings)) return;
        host.StartCoroutine(ResetSettings(selected, settings));
    }

    private IEnumerator ResetSettings(string id, ModSettings settings)
    {
        saves++;
        reset.Interactable(false);
        var controls = settingControls.ToArray();
        foreach (var control in controls) control(false);
        var task = Task.Run(() => { foreach (var setting in settings.Entries) setting.Reset(); });
        while (!task.IsCompleted) yield return null;
        if (task.IsFaulted) _ = task.Exception;
        saves--;
        foreach (var control in controls) control(true);
        if (reset != null) reset.Interactable(saves == 0);
        if (page != null && page.activeSelf && selected == id) ShowMod(id);
    }

    private static Sprite LoadIcon()
    {
        using var source = typeof(ModsMenu).Assembly.GetManifestResourceStream("Starframe.Bootstrap.Resources.starframe-mark-mono.png")!;
        using var bytes = new MemoryStream();
        source.CopyTo(bytes);
        var texture = new Texture2D(2, 2);
        ImageConversion.LoadImage(texture, bytes.ToArray());
        return Sprite.Create(texture, new Rect(0, 0, texture.width, texture.height), new Vector2(.5f, .5f));
    }

    public void Dispose()
    {
        if (page != null) Object.Destroy(page);
        if (entry != null) Object.Destroy(entry.gameObject);
        if (icon != null) { Object.Destroy(icon.texture); Object.Destroy(icon); }
        harmony.UnpatchSelf();
        if (current == this) current = null;
    }
}

internal sealed class SettingsRowSize : MonoBehaviour
{
    public TMP_Text Label = null!;
    public RectTransform Control = null!;
    public LayoutElement Layout = null!;
    public float Minimum;
    private TMP_Text[] values = System.Array.Empty<TMP_Text>();
    private void Awake() => values = Control.GetComponentsInChildren<TMP_Text>(true);
    private void LateUpdate()
    {
        float valueHeight = 40;
        foreach (var text in values) valueHeight = Math.Max(valueHeight, text.fontSize * 1.6f);
        float controlHeight = Math.Max(40, valueHeight + 12);
        if (Math.Abs(Control.rect.height - controlHeight) > .1f)
            Control.SetSizeWithCurrentAnchors(RectTransform.Axis.Vertical, controlHeight);
        float height = Math.Max(Minimum, Math.Max(Label.preferredHeight + 16, controlHeight + 16));
        if (Math.Abs(Layout.preferredHeight - height) > .1f) Layout.preferredHeight = height;
    }
}

internal sealed class SettingsButtonSize : MonoBehaviour
{
    public TMP_Text Label = null!;
    public LayoutElement Layout = null!;
    private RectTransform rect = null!;
    private void Awake() => rect = (RectTransform)transform;
    private void LateUpdate()
    {
        float available = Math.Max(100, ((RectTransform)transform.parent).rect.width - 40);
        var preferred = Label.GetPreferredValues(Label.text, available - 80, float.PositiveInfinity);
        float width = Math.Min(available, preferred.x + 80);
        float height = Math.Max(58, preferred.y + 24);
        if (Math.Abs(rect.rect.width - width) > .1f) rect.SetSizeWithCurrentAnchors(RectTransform.Axis.Horizontal, width);
        if (Math.Abs(Layout.preferredHeight - height) > .1f) Layout.preferredHeight = height;
    }
}
