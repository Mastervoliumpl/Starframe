#include <windows.h>
#include <commctrl.h>

static bool HighContrast()
{
    HIGHCONTRASTW contrast = {sizeof(contrast), 0, nullptr};
    return SystemParametersInfoW(SPI_GETHIGHCONTRAST, sizeof(contrast), &contrast, 0)
        && (contrast.dwFlags & HCF_HIGHCONTRASTON);
}

static bool DrawButton(const NMCUSTOMDRAW& draw)
{
    const HWND button = draw.hdr.hwndFrom;
    const UINT dpi = GetDpiForWindow(button);
    const bool disabled = !IsWindowEnabled(button);
    const bool focused = GetFocus() == button;
    const bool pressed = (draw.uItemState & CDIS_SELECTED) != 0;
    const bool hovered = (draw.uItemState & CDIS_HOT) != 0;
    const LONG style = GetWindowLongW(button, GWL_STYLE) & BS_TYPEMASK;
    const bool choice = style >= BS_CHECKBOX;
    const bool radio = style == BS_RADIOBUTTON || style == BS_AUTORADIOBUTTON;
    const LRESULT checked = choice ? SendMessageW(button, BM_GETCHECK, 0, 0) : BST_UNCHECKED;
    const bool primary = style == BS_DEFPUSHBUTTON;
    const COLORREF border = disabled ? RGB(71, 85, 105)
        : focused || primary ? RGB(251, 146, 60) : RGB(148, 163, 184);
    const COLORREF background = choice && !radio && checked != BST_UNCHECKED
        ? disabled ? RGB(148, 163, 184) : RGB(251, 146, 60)
        : pressed ? RGB(15, 23, 42)
        : hovered ? RGB(51, 65, 85) : RGB(30, 41, 59);
    const int saved = SaveDC(draw.hdc);
    if (!saved) return false;
    const int width = MulDiv(1, static_cast<int>(dpi), 96);
    const HPEN pen = CreatePen(PS_SOLID, width > 0 ? width : 1, border);
    const HBRUSH brush = CreateSolidBrush(background);
    const HBRUSH canvas = CreateSolidBrush(RGB(15, 23, 42));
    if (!pen || !brush || !canvas) {
        DeleteObject(canvas);
        DeleteObject(brush);
        DeleteObject(pen);
        RestoreDC(draw.hdc, saved);
        return false;
    }
    FillRect(draw.hdc, &draw.rc, canvas);
    SelectObject(draw.hdc, pen);
    SelectObject(draw.hdc, brush);
    RECT bounds = draw.rc;
    const int inset = width / 2;
    InflateRect(&bounds, -inset, -inset);
    if (choice) {
        const int size = MulDiv(16, static_cast<int>(dpi), 96);
        bounds.left += MulDiv(2, static_cast<int>(dpi), 96);
        bounds.top = (draw.rc.top + draw.rc.bottom - size) / 2;
        bounds.right = bounds.left + size;
        bounds.bottom = bounds.top + size;
    }
    const int radius = MulDiv(choice ? 3 : 6, static_cast<int>(dpi), 96);
    if (radio) Ellipse(draw.hdc, bounds.left, bounds.top, bounds.right, bounds.bottom);
    else RoundRect(draw.hdc, bounds.left, bounds.top, bounds.right, bounds.bottom, radius, radius);
    if (choice && checked != BST_UNCHECKED) {
        SelectObject(draw.hdc, GetStockObject(DC_PEN));
        SelectObject(draw.hdc, GetStockObject(DC_BRUSH));
        const COLORREF mark = radio
            ? disabled ? RGB(148, 163, 184) : RGB(251, 146, 60)
            : RGB(15, 23, 42);
        SetDCPenColor(draw.hdc, mark);
        SetDCBrushColor(draw.hdc, mark);
        const int size = bounds.right - bounds.left;
        if (radio) {
            RECT dot = bounds;
            InflateRect(&dot, -size / 4, -size / 4);
            Ellipse(draw.hdc, dot.left, dot.top, dot.right, dot.bottom);
        } else if (checked == BST_INDETERMINATE) {
            Rectangle(draw.hdc, bounds.left + size / 4, bounds.top + size * 2 / 5,
                bounds.right - size / 4, bounds.bottom - size * 2 / 5);
        } else {
            POINT tick[6] = {
                {bounds.left + size * 2 / 10, bounds.top + size * 5 / 10},
                {bounds.left + size * 4 / 10, bounds.top + size * 7 / 10},
                {bounds.left + size * 8 / 10, bounds.top + size * 3 / 10},
                {bounds.left + size * 8 / 10, bounds.top + size * 5 / 10},
                {bounds.left + size * 4 / 10, bounds.top + size * 9 / 10},
                {bounds.left + size * 2 / 10, bounds.top + size * 7 / 10}
            };
            Polygon(draw.hdc, tick, ARRAYSIZE(tick));
        }
    }
    if (choice) {
        const int textLeft = bounds.right + MulDiv(7, static_cast<int>(dpi), 96);
        bounds = draw.rc;
        bounds.left = textLeft;
    }
    const HFONT font = reinterpret_cast<HFONT>(SendMessageW(button, WM_GETFONT, 0, 0));
    if (font) SelectObject(draw.hdc, font);
    SetBkMode(draw.hdc, TRANSPARENT);
    SetTextColor(draw.hdc, disabled ? RGB(148, 163, 184) : RGB(248, 250, 252));
    wchar_t caption[256];
    GetWindowTextW(button, caption, ARRAYSIZE(caption));
    UINT flags = (choice ? DT_LEFT : DT_CENTER) | DT_VCENTER | DT_SINGLELINE;
    if (SendMessageW(button, WM_QUERYUISTATE, 0, 0) & UISF_HIDEACCEL) flags |= DT_HIDEPREFIX;
    DrawTextW(draw.hdc, caption, -1, &bounds, flags);
    if (focused) {
        RECT focus = bounds;
        DrawTextW(draw.hdc, caption, -1, &focus, flags | DT_CALCRECT);
        if (!choice) OffsetRect(&focus, (bounds.right - focus.right) / 2, 0);
        OffsetRect(&focus, 0, (bounds.bottom - focus.bottom) / 2);
        DrawFocusRect(draw.hdc, &focus);
    }
    RestoreDC(draw.hdc, saved);
    DeleteObject(canvas);
    DeleteObject(brush);
    DeleteObject(pen);
    return true;
}

static LRESULT CALLBACK DialogTheme(HWND window, UINT message, WPARAM wparam, LPARAM lparam,
    UINT_PTR id, DWORD_PTR)
{
    if (message == WM_NOTIFY && lparam && !HighContrast()) {
        const auto* notification = reinterpret_cast<const NMHDR*>(lparam);
        if (notification->code == NM_CUSTOMDRAW) {
            wchar_t kind[32];
            GetClassNameW(notification->hwndFrom, kind, ARRAYSIZE(kind));
            const LONG style = GetWindowLongW(notification->hwndFrom, GWL_STYLE) & BS_TYPEMASK;
            if (lstrcmpiW(kind, L"Button") == 0
                && (style <= BS_AUTO3STATE || style == BS_AUTORADIOBUTTON)) {
                const auto* draw = reinterpret_cast<const NMCUSTOMDRAW*>(lparam);
                if (draw->dwDrawStage == CDDS_PREPAINT && DrawButton(*draw)) {
                    return CDRF_SKIPDEFAULT;
                }
            }
        }
    }
    if (message == WM_NCDESTROY) RemoveWindowSubclass(window, DialogTheme, id);
    return DefSubclassProc(window, message, wparam, lparam);
}

static LRESULT CALLBACK BorderTheme(HWND window, UINT message, WPARAM wparam, LPARAM lparam,
    UINT_PTR id, DWORD_PTR group)
{
    if (group && message == WM_PAINT && !HighContrast()) {
        PAINTSTRUCT paint;
        const HDC dc = BeginPaint(window, &paint);
        const int saved = dc ? SaveDC(dc) : 0;
        const HPEN pen = CreatePen(PS_SOLID, 1, RGB(148, 163, 184));
        if (!saved || !pen) {
            if (saved) RestoreDC(dc, saved);
            DeleteObject(pen);
            EndPaint(window, &paint);
            InvalidateRect(window, nullptr, FALSE);
            return DefSubclassProc(window, message, wparam, lparam);
        }
        const HFONT font = reinterpret_cast<HFONT>(SendMessageW(window, WM_GETFONT, 0, 0));
        if (font) SelectObject(dc, font);
        TEXTMETRICW metrics;
        if (!GetTextMetricsW(dc, &metrics)) {
            metrics.tmHeight = 0;
            metrics.tmAveCharWidth = 0;
        }
        RECT bounds;
        GetClientRect(window, &bounds);
        bounds.top += metrics.tmHeight / 2;
        SelectObject(dc, pen);
        SelectObject(dc, GetStockObject(NULL_BRUSH));
        Rectangle(dc, bounds.left, bounds.top, bounds.right, bounds.bottom);
        wchar_t caption[256];
        GetWindowTextW(window, caption, ARRAYSIZE(caption));
        SetTextColor(dc, RGB(248, 250, 252));
        SetBkColor(dc, RGB(15, 23, 42));
        SetBkMode(dc, OPAQUE);
        TextOutW(dc, metrics.tmAveCharWidth, 0, caption, lstrlenW(caption));
        RestoreDC(dc, saved);
        DeleteObject(pen);
        EndPaint(window, &paint);
        return 0;
    }
    if (message == WM_NCDESTROY) RemoveWindowSubclass(window, BorderTheme, id);
    const LRESULT result = DefSubclassProc(window, message, wparam, lparam);
    if (!group && message == WM_NCPAINT && !HighContrast()) {
        RECT bounds;
        GetWindowRect(window, &bounds);
        OffsetRect(&bounds, -bounds.left, -bounds.top);
        POINT origin = {0, 0};
        ClientToScreen(window, &origin);
        RECT position;
        GetWindowRect(window, &position);
        const int inset = origin.x - position.left;
        const HDC dc = GetWindowDC(window);
        const HBRUSH background = CreateSolidBrush(RGB(30, 41, 59));
        const HBRUSH border = CreateSolidBrush(RGB(148, 163, 184));
        RECT edge = bounds;
        for (int i = 0; dc && border && background && i < inset; ++i) {
            FrameRect(dc, &edge, i == 0 ? border : background);
            InflateRect(&edge, -1, -1);
        }
        DeleteObject(border);
        DeleteObject(background);
        if (dc) ReleaseDC(window, dc);
    }
    return result;
}

static BOOL CALLBACK AttachDialog(HWND window, LPARAM)
{
    wchar_t kind[32];
    GetClassNameW(window, kind, ARRAYSIZE(kind));
    if (lstrcmpW(kind, L"#32770") == 0) SetWindowSubclass(window, DialogTheme, 1, 0);
    if (lstrcmpiW(kind, L"Button") == 0
        && (GetWindowLongW(window, GWL_STYLE) & BS_TYPEMASK) == BS_GROUPBOX)
        SetWindowSubclass(window, BorderTheme, 2, 1);
    if (lstrcmpiW(kind, L"Edit") == 0 || lstrcmpiW(kind, L"RichEdit20W") == 0
        || lstrcmpiW(kind, L"SysListView32") == 0)
        SetWindowSubclass(window, BorderTheme, 2, 0);
    return TRUE;
}

extern "C" void __cdecl Apply(HWND parent, int, wchar_t*, void*, void*)
{
    // Subclass callbacks must remain loaded until their windows are destroyed.
    HMODULE module;
    if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
        reinterpret_cast<LPCWSTR>(&Apply), &module)) return;
    const HWND header = GetDlgItem(parent, 1037);
    const HWND subtitle = GetDlgItem(parent, 1038);
    const HWND background = GetDlgItem(parent, 1034);
    if (header && subtitle && background) {
        RECT titleBounds;
        RECT subtitleBounds;
        GetWindowRect(header, &titleBounds);
        GetWindowRect(subtitle, &subtitleBounds);
        MapWindowPoints(nullptr, parent, reinterpret_cast<POINT*>(&titleBounds), 2);
        MapWindowPoints(nullptr, parent, reinterpret_cast<POINT*>(&subtitleBounds), 2);
        SetWindowPos(header, HWND_TOP, subtitleBounds.left, titleBounds.top,
            titleBounds.right - subtitleBounds.left, titleBounds.bottom - titleBounds.top,
            SWP_NOACTIVATE);
        // The full-width static backdrop must not repaint over the header labels.
        SetWindowLongW(background, GWL_STYLE,
            GetWindowLongW(background, GWL_STYLE) | WS_CLIPSIBLINGS);
        SetWindowPos(background, HWND_BOTTOM, 0, 0, 0, 0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    }
    SetWindowSubclass(parent, DialogTheme, 1, 0);
    EnumChildWindows(parent, AttachDialog, 0);
}

extern "C" BOOL WINAPI DllMain(HINSTANCE, DWORD, LPVOID)
{
    return TRUE;
}
