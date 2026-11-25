use crate::{
    commands::{AppState, blocking},
    db::Database,
    knowledge::SaveKnowledge,
    models::AppResult,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Mutex, atomic::Ordering},
};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct QuickbarConfig {
    enabled: bool,
}

impl Default for QuickbarConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl QuickbarConfig {
    fn load(directory: &Path) -> AppResult<Self> {
        match fs::read(directory.join("quickbar.json")) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|err| format!("无法解析全局快捷键设置：{err}")),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(format!("无法读取全局快捷键设置：{err}")),
        }
    }

    fn save(&self, directory: &Path) -> AppResult<()> {
        let bytes = serde_json::to_vec(self).map_err(|err| err.to_string())?;
        let temp = directory.join(format!("quickbar-{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| -> io::Result<()> {
            fs::create_dir_all(directory)?;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temp, directory.join("quickbar.json"))
        })();
        result.map_err(|err| {
            let _ = fs::remove_file(&temp);
            format!("无法保存全局快捷键设置：{err}")
        })
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuickbarStatus {
    pub enabled: bool,
    pub registered: bool,
    pub shortcut: String,
    pub error: Option<String>,
}

pub(crate) struct QuickbarState {
    directory: PathBuf,
    status: Mutex<QuickbarStatus>,
    window: Mutex<()>,
    #[cfg(windows)]
    hotkey: Option<windows::Hotkey>,
}

#[cfg(not(windows))]
const UNSUPPORTED: &str = "全局快捷键仅在 Windows 桌面版可用。";

impl QuickbarState {
    pub(crate) fn start(_app: AppHandle, directory: &Path) -> Self {
        let config = QuickbarConfig::load(directory);
        let enabled = config.as_ref().map_or_else(
            |_| QuickbarConfig::default().enabled,
            |config| config.enabled,
        );
        #[cfg(windows)]
        let (hotkey, registered, error) = if cfg!(debug_assertions)
            && (std::env::var_os("CORTEXOS_TEST_DATA_DIR").is_some()
                || std::env::var_os("CORTEXOS_TEST_CDP_PORT").is_some())
        {
            (None, false, None)
        } else {
            match windows::Hotkey::start(_app, enabled) {
                Ok((hotkey, (registered, error))) => (Some(hotkey), registered, error),
                Err(error) => (None, false, Some(error)),
            }
        };
        #[cfg(windows)]
        let error = error.or_else(|| config.err());
        #[cfg(not(windows))]
        let (registered, error) = (false, Some(UNSUPPORTED.into()));

        Self {
            directory: directory.to_owned(),
            status: Mutex::new(QuickbarStatus {
                enabled,
                registered,
                shortcut: "Alt+Space".into(),
                error,
            }),
            window: Mutex::new(()),
            #[cfg(windows)]
            hotkey,
        }
    }

    fn set_enabled(&self, enabled: bool) -> AppResult<QuickbarStatus> {
        let mut status = self.status.lock().map_err(|err| err.to_string())?;
        QuickbarConfig { enabled }.save(&self.directory)?;
        status.enabled = enabled;
        #[cfg(windows)]
        if let Some(hotkey) = &self.hotkey {
            (status.registered, status.error) = hotkey.sync(enabled);
        }
        Ok(status.clone())
    }

    pub(crate) fn shutdown(&self) {
        #[cfg(windows)]
        if let Some(hotkey) = &self.hotkey {
            hotkey.shutdown();
        }
    }
}

#[cfg(windows)]
mod windows {
    use super::{AppHandle, AppResult, Mutex, toggle_window};
    use std::{
        ptr::null_mut,
        sync::{
            Arc,
            mpsc::{SyncSender, sync_channel},
        },
        thread,
    };
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct Msg {
        hwnd: *mut core::ffi::c_void,
        message: u32,
        w_param: usize,
        l_param: isize,
        time: u32,
        pt: Point,
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn RegisterHotKey(hwnd: *mut core::ffi::c_void, id: i32, fs_modifiers: u32, vk: u32)
        -> i32;
        fn UnregisterHotKey(hwnd: *mut core::ffi::c_void, id: i32) -> i32;
        fn GetMessageW(lp_msg: *mut Msg, hwnd: *mut core::ffi::c_void, min: u32, max: u32) -> i32;
        fn PeekMessageW(
            lp_msg: *mut Msg,
            hwnd: *mut core::ffi::c_void,
            min: u32,
            max: u32,
            remove: u32,
        ) -> i32;
        fn PostThreadMessageW(id_thread: u32, msg: u32, w_param: usize, l_param: isize) -> i32;
    }

    const HOTKEY_ID: i32 = 0x4358;
    const MOD_ALT: u32 = 0x0001;
    const MOD_NOREPEAT: u32 = 0x4000;
    const VK_SPACE: u32 = 0x20;
    const WM_HOTKEY: u32 = 0x0312;
    const WM_QUIT: u32 = 0x0012;
    const WM_APP_SYNC: u32 = 0x8001;

    type Registration = (bool, Option<String>);

    #[derive(Default)]
    struct ThreadState {
        id: Option<u32>,
        request: Option<(bool, SyncSender<Registration>)>,
    }

    pub(super) struct Hotkey(Arc<Mutex<ThreadState>>);

    impl Hotkey {
        pub(super) fn start(app: AppHandle, enabled: bool) -> AppResult<(Self, Registration)> {
            let shared = Arc::new(Mutex::new(ThreadState::default()));
            let worker = Arc::clone(&shared);
            let (ready, started) = sync_channel(1);
            thread::Builder::new()
                .name("cortexos-quickbar-hotkey".into())
                .spawn(move || {
                    // MSG has a valid all-zero representation. PeekMessage creates this thread's queue.
                    let mut msg: Msg = unsafe { std::mem::zeroed() };
                    let id = unsafe {
                        PeekMessageW(&mut msg, null_mut(), 0, 0, 0);
                        GetCurrentThreadId()
                    };
                    worker.lock().unwrap().id = Some(id);
                    let mut registered = false;
                    let _ = ready.send(update_registration(&mut registered, enabled));

                    while unsafe { GetMessageW(&mut msg, null_mut(), 0, 0) } > 0 {
                        if registered
                            && msg.message == WM_HOTKEY
                            && msg.w_param == HOTKEY_ID as usize
                        {
                            // WebView creation can wait on the UI thread. Keep the Win32 queue responsive.
                            let app = app.clone();
                            tauri::async_runtime::spawn_blocking(move || {
                                let _ = toggle_window(&app);
                            });
                        } else if msg.message == WM_APP_SYNC {
                            if let Some((enabled, reply)) = worker.lock().unwrap().request.take() {
                                let _ = reply.send(update_registration(&mut registered, enabled));
                            }
                        }
                    }
                    if registered {
                        unsafe { UnregisterHotKey(null_mut(), HOTKEY_ID) };
                    }
                    *worker.lock().unwrap() = ThreadState::default();
                })
                .map_err(|err| format!("无法启动全局快捷键线程：{err}"))?;
            let registration = started
                .recv()
                .map_err(|err| format!("无法初始化全局快捷键：{err}"))?;
            Ok((Self(shared), registration))
        }

        pub(super) fn sync(&self, enabled: bool) -> Registration {
            let unavailable = || {
                (
                    false,
                    Some("全局快捷键线程不可用，请重启应用后重试。".into()),
                )
            };
            let (reply, result) = sync_channel(1);
            {
                let mut state = self.0.lock().unwrap();
                let Some(id) = state.id else {
                    return unavailable();
                };
                state.request = Some((enabled, reply));
                if unsafe { PostThreadMessageW(id, WM_APP_SYNC, 0, 0) } == 0 {
                    state.request = None;
                    return unavailable();
                }
            }
            result.recv().unwrap_or_else(|_| unavailable())
        }

        pub(super) fn shutdown(&self) {
            let mut state = self.0.lock().unwrap();
            if let Some(id) = state.id.take() {
                unsafe { PostThreadMessageW(id, WM_QUIT, 0, 0) };
            }
            state.request = None;
        }
    }

    fn update_registration(registered: &mut bool, enabled: bool) -> Registration {
        if enabled && !*registered {
            *registered =
                unsafe { RegisterHotKey(null_mut(), HOTKEY_ID, MOD_ALT | MOD_NOREPEAT, VK_SPACE) }
                    != 0;
            if !*registered {
                return (false, Some("全局快捷键 Alt+Space 已被其他程序占用，可关闭冲突热键后重试，或直接在应用内呼出。".into()));
            }
        } else if !enabled && *registered {
            if unsafe { UnregisterHotKey(null_mut(), HOTKEY_ID) } == 0 {
                return (
                    true,
                    Some(format!(
                        "无法释放全局快捷键：{}",
                        std::io::Error::last_os_error()
                    )),
                );
            }
            *registered = false;
        }
        (*registered, None)
    }
}

pub(crate) fn toggle_window(app: &AppHandle) -> AppResult<bool> {
    let Some(state) = app.try_state::<QuickbarState>() else {
        return Ok(false);
    };
    let _window = state.window.lock().map_err(|err| err.to_string())?;
    let main = app.state::<AppState>();
    if main.closing.load(Ordering::SeqCst) {
        return Ok(false);
    }
    let win = if let Some(win) = app.get_webview_window("quickbar") {
        if win.is_visible().unwrap_or(false) && win.is_focused().unwrap_or(false) {
            win.hide().map_err(|err| err.to_string())?;
            return Ok(false);
        }
        let _ = win.unminimize();
        let _ = win.center();
        win.show().map_err(|err| err.to_string())?;
        win
    } else {
        tauri::WebviewWindowBuilder::new(
            app,
            "quickbar",
            tauri::WebviewUrl::App("index.html#/quickbar".into()),
        )
        .title("CortexOS 随手记与秒问")
        .inner_size(640.0, 136.0)
        .min_inner_size(520.0, 120.0)
        .max_inner_size(760.0, 520.0)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .center()
        .visible(true)
        .focused(true)
        .build()
        .map_err(|err| format!("无法打开随手记浮窗：{err}"))?
    };
    if main.closing.load(Ordering::SeqCst) {
        let _ = win.destroy();
        return Ok(false);
    }
    let _ = win.set_focus();
    let _ = app.emit_to("quickbar", "quickbar-shown", ());
    Ok(true)
}

#[tauri::command]
pub(crate) async fn quickbar_status(state: State<'_, QuickbarState>) -> AppResult<QuickbarStatus> {
    state
        .status
        .lock()
        .map(|status| status.clone())
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub(crate) async fn set_quickbar_enabled(
    state: State<'_, QuickbarState>,
    enabled: bool,
) -> AppResult<QuickbarStatus> {
    state.set_enabled(enabled)
}

#[tauri::command]
pub(crate) async fn toggle_quickbar(app: AppHandle) -> AppResult<bool> {
    toggle_window(&app)
}

#[tauri::command]
pub(crate) async fn hide_quickbar(app: AppHandle) -> AppResult<()> {
    if let Some(win) = app.get_webview_window("quickbar") {
        win.hide().map_err(|err| err.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn resize_quickbar(app: AppHandle, height: f64) -> AppResult<()> {
    if let Some(win) = app.get_webview_window("quickbar") {
        if height.is_nan() {
            return Err("浮窗高度无效。".into());
        }
        win.set_size(tauri::Size::Logical(tauri::LogicalSize::new(
            640.0,
            height.clamp(120.0, 480.0),
        )))
        .map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn save_note_to_db(db: &Database, workspace_id: &str, content: &str) -> AppResult<String> {
    let content = content.trim();
    let content = content.strip_prefix('/').unwrap_or(content).trim();
    if !(1..=20000).contains(&content.chars().count()) {
        return Err("随手记正文需为 1–20000 字。".into());
    }
    let title: String = content
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default()
        .trim_start_matches(|ch: char| ch.is_whitespace() || matches!(ch, '#' | '*' | '-' | '>'))
        .chars()
        .take(48)
        .collect();
    let title = if title.is_empty() {
        "随手记".into()
    } else {
        title
    };
    db.save_knowledge(SaveKnowledge {
        workspace_id: workspace_id.into(),
        node_id: None,
        expected_revision: None,
        title,
        kind: "knowledge".into(),
        content: content.into(),
        change_reason: "全局随手记快速入库".into(),
        source_message_id: None,
        source_excerpt: None,
    })
}

#[tauri::command]
pub(crate) async fn quickbar_save_note(
    app: AppHandle,
    state: State<'_, AppState>,
    workspace_id: String,
    content: String,
) -> AppResult<String> {
    let workspace = workspace_id.clone();
    let node_id = blocking(state.database.clone()?, move |db| {
        save_note_to_db(db, &workspace, &content)
    })
    .await?;
    let _ = app.emit(
        "quickbar-knowledge-saved",
        serde_json::json!({"workspaceId": workspace_id, "nodeId": node_id}),
    );
    Ok(node_id)
}

#[tauri::command]
pub(crate) async fn open_in_main_window(
    app: AppHandle,
    workspace_id: String,
    conversation_id: Option<String>,
    node_id: Option<String>,
) -> AppResult<()> {
    hide_quickbar(app.clone()).await?;
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.unminimize();
        let _ = main.show();
        let _ = main.set_focus();
    }
    let _ = app.emit("quickbar-navigate", serde_json::json!({"workspaceId": workspace_id, "conversationId": conversation_id, "nodeId": node_id}));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quickbar_config_persists_and_note_saves_searchable_knowledge() {
        let directory =
            std::env::temp_dir().join(format!("cortexos-quickbar-{}", uuid::Uuid::new_v4()));
        assert!(QuickbarConfig::default().enabled);
        assert!(QuickbarConfig::load(&directory).unwrap().enabled);
        QuickbarConfig { enabled: false }.save(&directory).unwrap();
        assert!(!QuickbarConfig::load(&directory).unwrap().enabled);
        QuickbarConfig { enabled: true }.save(&directory).unwrap();
        assert!(QuickbarConfig::load(&directory).unwrap().enabled);

        {
            let db = Database::open(&directory).unwrap();
            let ws = db.create_workspace("QuickBar").unwrap().id;
            let text = "灵感：周三评审 2D 思维导图连线推荐";
            let id = save_note_to_db(&db, &ws, &format!("/{text}")).unwrap();
            let detail = db.knowledge_detail(&ws, &id).unwrap();
            assert_eq!(detail.node.title, text);
            assert_eq!(detail.node.effective_version, Some(1));
            assert_eq!(detail.versions[0].content, text);
            let conn = db.connection().unwrap();
            let report = crate::retrieval::search(&conn, &ws, "思维导图").unwrap();
            assert_eq!(report["candidates"][0]["nodeId"], id);
        }
        fs::remove_dir_all(directory).unwrap();
    }
}
