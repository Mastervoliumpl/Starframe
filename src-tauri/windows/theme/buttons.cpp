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
    const bool primary = (GetWindowLongW(button, GWL_STYLE) & BS_TYPEMASK) == BS_DEFPUSHBUTTON;
    const COLORREF border = disabled ? RGB(71, 85, 105)
        : focused || primary ? RGB(251, 146, 60) : RGB(148, 163, 184);
    const COLORREF background = pressed ? RGB(15, 23, 42)
        : hovered ? RGB(51, 65, 85) : RGB(30, 41, 59);
    const int saved = SaveDC(draw.hdc);
    if (!saved) return false;
    const int width = MulDiv(focused ? 2 : 1, static_cast<int>(dpi), 96);
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
    const int radius = MulDiv(6, static_cast<int>(dpi), 96);
    RoundRect(draw.hdc, bounds.left, bounds.top, bounds.right, bounds.bottom, radius, radius);
    const HFONT font = reinterpret_cast<HFONT>(SendMessageW(button, WM_GETFONT, 0, 0));
    if (font) SelectObject(draw.hdc, font);
    SetBkMode(draw.hdc, TRANSPARENT);
    SetTextColor(draw.hdc, disabled ? RGB(148, 163, 184) : RGB(248, 250, 252));
    wchar_t caption[256];
    GetWindowTextW(button, caption, ARRAYSIZE(caption));
    UINT flags = DT_CENTER | DT_VCENTER | DT_SINGLELINE;
    if (SendMessageW(button, WM_QUERYUISTATE, 0, 0) & UISF_HIDEACCEL) flags |= DT_HIDEPREFIX;
    DrawTextW(draw.hdc, caption, -1, &bounds, flags);
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
            if (lstrcmpiW(kind, L"Button") == 0 && style <= BS_DEFPUSHBUTTON) {
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
    SetWindowSubclass(parent, DialogTheme, 1, 0);
    EnumChildWindows(parent, AttachDialog, 0);
}

extern "C" BOOL WINAPI DllMain(HINSTANCE, DWORD, LPVOID)
{
    return TRUE;
}
