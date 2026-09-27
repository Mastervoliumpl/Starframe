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
    private readonly MenuLifecycle lifecycle = new();
    private GameObject? page;
    private PanelButton? entry;
    private Transform list = null!;
    private GameObject header = null!, body = null!, button = null!, toggle = null!, input = null!, choice = null!;
    private SidebarReveal sidebarReveal = null!;
    private PanelManager windowPanels = null!;
    private RectTransform sidebarIndicator = null!;
    private TMP_Text descriptionTitle = null!, description = null!;
    private Vector2 panelSize, panelPosition, buttonsSize, buttonsPosition;
    private ButtonManager back = null!, reset = null!;
    private Sprite? icon;
    private Coroutine? reveal;
    private bool opening;
    private string? selected;
    private int saves;
    private float listScroll = 1;

    public ModsMenu(MonoBehaviour host, ActivationSession session, byte[] report)
    {
        this.host = host;
        this.session = session;
        using var document = Contracts.Read(report, "report");
        outcomes = document.RootElement.GetProperty("mods").EnumerateArray().ToDictionary(m => m.GetProperty("modId").GetString()!, m => m.Clone());
        try
        {
            harmony.Patch(AccessTools.Method(typeof(InterfaceManager), nameof(InterfaceManager.TransitionTo)),
                prefix: new HarmonyMethod(typeof(ModsMenu), nameof(HideForTransition)));
            harmony.Patch(AccessTools.Method(typeof(InterfaceManager), nameof(InterfaceManager.ToggleInGameMenu)),
                prefix: new HarmonyMethod(typeof(ModsMenu), nameof(HandleGameEscape)));
            harmony.Patch(AccessTools.Method(typeof(SidebarReveal), nameof(SidebarReveal.Hide)),
                prefix: new HarmonyMethod(typeof(ModsMenu), nameof(AllowSidebarHide)));
            current = this;
        }
        catch { harmony.UnpatchSelf(); throw; }
    }

    private static void HideForTransition()
    {
        if (current?.page == null) return;
        current.page.SetActive(false);
        current.entry?.SetSelected(false);
    }

    private static bool HandleGameEscape()
    {
        if (current == null) return true;
        if (current.lifecycle.HandledEscape(Time.frameCount)) return false;
        if (current.page == null || !current.page.activeInHierarchy) return true;
        current.lifecycle.Escape(Time.frameCount, current.Back);
        return false;
    }

    private static bool AllowSidebarHide(SidebarReveal __instance) =>
        current == null || !current.opening || __instance != current.sidebarReveal;

    public void Tick()
    {
        var sidebar = SideBarInterface.Instance;
        var settings = SanctuaryUI.SettingsInterface.Instance;
        if (sidebar != null && settings != null && InterfaceManager.Instance != null)
            lifecycle.Build(sidebar, settings, () => Build(sidebar, settings), ClearPage);
        if (page == null || !page.activeSelf) return;
        if (Input.GetKeyDown(KeyCode.Escape)) { lifecycle.Escape(Time.frameCount, Back); return; }
        if (Input.GetKeyDown(KeyCode.Tab))
        {
            var available = focus.Where(g => g != null && g.activeInHierarchy && (g.GetComponent<Selectable>()?.IsInteractable() ?? true)).ToList();
            if (available.Count == 0) return;
            int index = available.IndexOf(EventSystem.current.currentSelectedGameObject);
            int step = Input.GetKey(KeyCode.LeftShift) || Input.GetKey(KeyCode.RightShift) ? -1 : 1;
            Select(available[(index + step + available.Count) % available.Count]);
        }
    }

    private void Build(SideBarInterface sidebar, SanctuaryUI.SettingsInterface settings)
    {
        var original = settings.transform;
        RequireTemplates(original);
        windowPanels = InterfaceManager.Instance.GetComponent<PanelManager>();
        sidebarIndicator = (RectTransform)AccessTools.Field(typeof(PanelManager), "indicator").GetValue(windowPanels);
        if (sidebarIndicator == null) throw new InvalidOperationException("The game's sidebar selection indicator is unavailable.");
        sidebarReveal = sidebar.GetComponentInParent<SidebarReveal>();
        if (sidebarReveal == null) throw new InvalidOperationException("The game's sidebar reveal control is unavailable.");
        var template = sidebar.settingsButton;
        if (template == null) throw new InvalidOperationException("The game's Settings sidebar button is unavailable.");
        var staging = new GameObject("StarframeMenuTemplates");
        staging.SetActive(false);
        try
        {
            // An inactive parent prevents the clone's native Awake from replacing SettingsInterface.Instance.
            page = Object.Instantiate(original.gameObject, staging.transform);
            page.SetActive(false);
            Object.DestroyImmediate(page.GetComponent<SanctuaryUI.SettingsInterface>());
            page.transform.SetParent(original.parent, false);
            entry = Object.Instantiate(template.gameObject, staging.transform).GetComponent<PanelButton>();
            var copiedIndicator = entry.transform.Find(sidebarIndicator.name);
            if (copiedIndicator != null) Object.DestroyImmediate(copiedIndicator.gameObject);
            entry.gameObject.SetActive(false);
            entry.transform.SetParent(template.transform.parent, false);
        }
        finally { Object.Destroy(staging); }
        page.name = "StarframeMods";
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
        choice = Keep(list.Find("Window Mode"), templates.transform);
        toggle = Keep(page.transform.Find("Content/Panels/Controls/Content/List/Layout Group/EdgePanToggle"), templates.transform);
        foreach (Transform panel in page.transform.Find("Content/Panels"))
            if (panel.name is "General" or "Audio" or "Controls") panel.gameObject.SetActive(false);
        page.transform.Find("Content/Categories").gameObject.SetActive(false);
        var title = page.transform.Find("Content/Panel Header");
        foreach (var localized in title.GetComponentsInChildren<LocalizedObject>(true)) Object.DestroyImmediate(localized);
        foreach (var text in title.GetComponentsInChildren<TMP_Text>(true)) { text.text = "Mods"; text.richText = false; }
        icon ??= LoadIcon();
        var panels = page.GetComponent<PanelManager>();
        panels.panels = new List<PanelManager.PanelItem> { new() { panelName = "Mods", panelObject = graphics.GetComponent<Animator>() } };
        panels.currentPanelIndex = 0;
        var area = page.transform.Find("Content/Description Area");
        foreach (var manager in area.GetComponentsInChildren<SettingsDescriptionManager>(true)) Object.DestroyImmediate(manager);
        foreach (var localized in area.GetComponentsInChildren<LocalizedObject>(true)) Object.DestroyImmediate(localized);
        descriptionTitle = area.Find("Title").GetComponent<TMP_Text>();
        descriptionTitle.richText = false;
        description = Object.Instantiate(body, area).GetComponent<TMP_Text>();
        description.gameObject.name = "Description";
        description.gameObject.SetActive(true);
        area.Find("Cover/Image Parent/Image").GetComponent<Image>().sprite = icon;
        var panelRect = (RectTransform)page.transform.Find("Content/Panels");
        var buttonsRect = (RectTransform)page.transform.Find("Content/Buttons");
        panelSize = panelRect.sizeDelta; panelPosition = panelRect.anchoredPosition;
        buttonsSize = buttonsRect.sizeDelta; buttonsPosition = buttonsRect.anchoredPosition;
        back = page.transform.Find("Content/Buttons/BackButton").GetComponent<ButtonManager>();
        Configure(back, "Back", Back);
        reset = page.transform.Find("Content/Buttons/Reset Settings Button").GetComponent<ButtonManager>();
        Configure(reset, "Reset this mod", Reset);
        entry.name = "StarframeModsButton";
        entry.transform.SetSiblingIndex(template.transform.GetSiblingIndex() + 1);
        Configure(entry, "Mods");
        entry.buttonIcon = entry.selectedIcon = icon;
        entry.onClick.AddListener(Open);
        entry.gameObject.SetActive(true);
        entry.UpdateUI();
        entry.AddUINavigation();
        entry.GetComponentInParent<PanelButtonDimmer>()?.FetchButtons();
        Clear();
    }

    private static void RequireTemplates(Transform original)
    {
        foreach (string path in new[]
        {
            "Content/Panels/Graphics/Content/List/Layout Group/Display Header",
            "Content/Panels/Graphics/Content/List/Layout Group/ApplyButton",
            "Content/Panels/Graphics/Content/List/Layout Group/UI Scale/Slider/Text Input",
            "Content/Panels/Graphics/Content/List/Layout Group/Window Mode/Horizontal Selector",
            "Content/Panels/Controls/Content/List/Layout Group/EdgePanToggle/Switch",
            "Content/Categories", "Content/Panel Header", "Content/Description Area/Description", "Content/Description Area/Title", "Content/Description Area/Cover/Image Parent/Image",
            "Content/Buttons/BackButton", "Content/Buttons/Reset Settings Button"
        })
            if (original.Find(path) == null) throw new InvalidOperationException("The game's Settings menu is missing " + path + ".");
        if (original.GetComponent<PanelManager>() == null) throw new InvalidOperationException("The game's Settings panel manager is unavailable.");
        if (original.GetComponent<Animator>()?.runtimeAnimatorController == null)
            throw new InvalidOperationException("The game's Settings window animator is unavailable.");
    }

    private void ClearPage()
    {
        if (reveal != null) { host.StopCoroutine(reveal); reveal = null; }
        if (entry != null && sidebarIndicator != null && sidebarIndicator.IsChildOf(entry.transform))
        {
            windowPanels.StopCoroutine("MoveIndicatorToParent");
            windowPanels.StopCoroutine("SetIndicatorHeight");
            sidebarIndicator.SetParent(entry.transform.parent, true);
            sidebarIndicator.sizeDelta = new Vector2(sidebarIndicator.sizeDelta.x, 0);
        }
        if (page != null) { page.SetActive(false); Object.Destroy(page); }
        if (entry != null) { entry.gameObject.SetActive(false); Object.Destroy(entry.gameObject); }
        page = null;
        entry = null;
        focus.Clear();
        settingControls.Clear();
        selected = null;
        listScroll = 1;
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
        // The Background transition queues Hide; a same-frame Show does not cancel that native animation.
        opening = true;
        try { InterfaceManager.Instance.TransitionTo(InterfaceManager.Window.Background); }
        finally { opening = false; }
        // Background has no sidebar button, so its transition collapses the shared indicator.
        windowPanels.StopCoroutine("MoveIndicatorToParent");
        windowPanels.StartCoroutine("MoveIndicatorToParent", entry!.transform);
        entry!.SetSelected(true);
        page!.SetActive(true);
        var animator = page.GetComponent<Animator>();
        if (reveal != null) host.StopCoroutine(reveal);
        animator.enabled = true;
        animator.SetFloat("AnimSpeed", 1);
        animator.Play("In Top", 0, 0);
        reveal = host.StartCoroutine(FinishReveal(animator));
        ShowList();
    }

    private IEnumerator FinishReveal(Animator animator)
    {
        yield return new WaitForSecondsRealtime(BeamUIInternalTools.GetAnimatorClipLength(animator, "MainPanel_InTop"));
        if (animator != null) animator.enabled = false;
        reveal = null;
    }

    private void Back()
    {
        if (selected != null) { ShowList(); return; }
        page!.SetActive(false);
        InterfaceManager.Instance.TransitionTo(InterfaceManager.Window.Home);
        if (entry != null) Select(entry.gameObject);
    }

    private void Clear()
    {
        foreach (Transform child in list) { child.gameObject.SetActive(false); Object.Destroy(child.gameObject); }
        focus.Clear();
        settingControls.Clear();
    }

    private void ShowList()
    {
        string? previous = selected;
        selected = null;
        GameObject? restore = null;
        Clear();
        SetDetails(false);
        reset.gameObject.SetActive(false);
        Text("Mods", heading: true);
        Text("The current game session. Change mods in Starframe, then restart the game.");
        var active = session.InstalledMods.Where(m => outcomes.ContainsKey(m.GetProperty("modId").GetString()!)).ToArray();
        bool Loaded(JsonElement mod) => outcomes[mod.GetProperty("modId").GetString()!].GetProperty("outcome").GetString() == "loaded";
        if (!active.Any(Loaded)) Text("No mods are running in this session.");
        foreach (var mod in active.Where(Loaded)) AddMod(mod);
        if (active.Any(m => !Loaded(m)))
        {
            Text("Could not load", heading: true);
            Text("Open a mod for details. Fix the setup in Starframe, then restart the game.");
            foreach (var mod in active.Where(m => !Loaded(m))) AddMod(mod);
        }
        FinishPage(restore, previous == null ? 1 : listScroll);

        void AddMod(JsonElement mod)
        {
            string id = mod.GetProperty("modId").GetString()!;
            var row = AddButton(mod.GetProperty("name").GetString()!, () => ShowMod(id));
            if (id == previous) restore = row.gameObject;
        }
    }

    private void ShowMod(string id)
    {
        if (selected == null) listScroll = list.GetComponentInParent<ScrollRect>().verticalNormalizedPosition;
        selected = id;
        Clear();
        SetDetails(true);
        var mod = session.InstalledMods.First(m => m.GetProperty("modId").GetString() == id);
        Describe(mod.GetProperty("name").GetString()!, "Select a setting to see its description and default value.");
        Text(mod.GetProperty("name").GetString()!, heading: true);
        bool loaded = session.Settings.TryGetValue(id, out var settings);
        reset.gameObject.SetActive(loaded && settings!.Entries.Count > 0);
        if (outcomes[id].GetProperty("outcome").GetString() != "loaded") Text(outcomes[id].GetProperty("message").GetString()!);
        else if (!loaded || settings!.Entries.Count == 0) Text("This mod has not registered any settings.");
        else foreach (var setting in settings.Entries) AddSetting(setting);
        FinishPage();
    }

    private void FinishPage(GameObject? restore = null, float scroll = 1)
    {
        focus.Add(back.gameObject);
        if (reset.gameObject.activeSelf) focus.Add(reset.gameObject);
        foreach (Transform sibling in entry!.transform.parent)
            if (sibling.GetComponent<PanelButton>() is { isInteractable: true } sidebarButton) focus.Add(sidebarButton.gameObject);
        Canvas.ForceUpdateCanvases();
        list.GetComponentInParent<ScrollRect>().verticalNormalizedPosition = scroll;
        reset.Interactable(saves == 0);
        Select(restore != null ? restore : focus[0]);
    }

    private void SetDetails(bool details)
    {
        page!.transform.Find("Content/Description Area").gameObject.SetActive(details);
        var panels = (RectTransform)page.transform.Find("Content/Panels");
        panels.sizeDelta = details ? panelSize : new Vector2(-70, panelSize.y);
        panels.anchoredPosition = details ? panelPosition : new Vector2(0, panelPosition.y);
        var buttons = (RectTransform)page.transform.Find("Content/Buttons");
        buttons.sizeDelta = details ? buttonsSize : new Vector2(-70, buttonsSize.y);
        buttons.anchoredPosition = details ? buttonsPosition : new Vector2(0, buttonsPosition.y);
    }

    private void Describe(string title, string text)
    {
        descriptionTitle.text = title;
        description.text = text;
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
        control.enableIcon = false;
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
            var row = Object.Instantiate(choice, list);
            row.transform.Find("Text").GetComponent<TMP_Text>().text = setting.Label;
            var control = row.GetComponentInChildren<HorizontalSelector>(true);
            control.useLocalization = control.saveSelected = control.invokeOnAwake = false;
            control.items = choices.Select(value => new HorizontalSelector.Item { itemTitle = value }).ToList();
            control.defaultIndex = Math.Max(0, Array.IndexOf(choices, setting.Text));
            control.onValueChanged = new HorizontalSelector.HorizontalSelectorEvent();
            control.onValueChanged.AddListener(index => save(choices[index]));
            var arrows = control.GetComponentsInChildren<ButtonManager>(true);
            foreach (var arrow in arrows) arrow.useUINavigation = true;
            enable = value => { foreach (var arrow in arrows) if (arrow != null) arrow.Interactable(value); };
            refresh = () => { if (control != null) { control.index = Math.Max(0, Array.IndexOf(choices, setting.Text)); control.UpdateUI(); } };
            FitRow(row, (RectTransform)control.transform);
            row.SetActive(true);
            foreach (var arrow in arrows) focus.Add(arrow.gameObject);
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
        Action showDescription = () => Describe(setting.Label, setting.Description + "\n\nDefault: " + setting.DefaultText + ".\n" + setting.ApplyDescription);
        var rowObject = focus[^1].transform;
        while (rowObject.parent != list) rowObject = rowObject.parent;
        rowObject.gameObject.AddComponent<ModSettingDescription>().Show = showDescription;
        foreach (var target in focus.Where(target => target.transform.IsChildOf(rowObject)))
            if (target.transform != rowObject) target.AddComponent<ModSettingDescription>().Show = showDescription;
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
        yield return null;
        var task = SettingsTask(setting.RequiresMainThread, () => setting.SaveText(value));
        while (!task.IsCompleted) yield return null;
        if (task.IsFaulted) _ = task.Exception;
        saves--;
        enable(true);
        refresh();
        if (reset != null) reset.Interactable(saves == 0);
        if (status != null) status.text = task.IsFaulted ? setting.Error : setting.RestartRequired ? "Saved. Restart required." : "Saved";
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
        yield return null;
        var task = SettingsTask(settings.Entries.Any(s => s.RequiresMainThread), () => { foreach (var setting in settings.Entries) setting.Reset(); });
        while (!task.IsCompleted) yield return null;
        if (task.IsFaulted) _ = task.Exception;
        saves--;
        foreach (var control in controls) control(true);
        if (reset != null) reset.Interactable(saves == 0);
        if (page != null && page.activeSelf && selected == id) ShowMod(id);
    }

    private static Task SettingsTask(bool mainThread, Action action)
    {
        // BepInEx setting events can call Unity APIs, so their setters must stay on the game thread.
        if (!mainThread) return Task.Run(action);
        try { action(); return Task.CompletedTask; }
        catch (Exception error) { return Task.FromException(error); }
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
        ClearPage();
        if (icon != null) { Object.Destroy(icon.texture); Object.Destroy(icon); }
        harmony.UnpatchSelf();
        if (current == this) current = null;
    }
}

internal sealed class ModSettingDescription : MonoBehaviour, IPointerEnterHandler, ISelectHandler
{
    public Action Show = null!;
    public void OnPointerEnter(PointerEventData eventData) => Show();
    public void OnSelect(BaseEventData eventData) => Show();
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
