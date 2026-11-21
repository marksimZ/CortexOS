use crate::{
    commands::AppState,
    models::{AppResult, new_id},
};
use reqwest::{Url, redirect::Policy};
use serde::{Deserialize, Serialize};
use std::{sync::atomic::Ordering, time::Duration};
use tauri::{AppHandle, Manager, State, WebviewWindow, ipc::Channel};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio::sync::Mutex;

const MAX_INSTALLER_BYTES: usize = 128 * 1024 * 1024;
const BUSY: &str = "已有更新操作正在处理，请稍后再试。";
const EXPIRED: &str = "当前更新已失效，请重新检查。";

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum UpdateChannel {
    Stable,
    Preview,
}

impl UpdateChannel {
    fn endpoint(self, app: &AppHandle) -> Option<(Url, String)> {
        #[cfg(debug_assertions)]
        if let (Ok(origin), Ok(key)) = (
            std::env::var("CORTEXOS_TEST_UPDATE_ORIGIN"),
            std::env::var("CORTEXOS_TEST_UPDATE_PUBKEY"),
        ) {
            let url = Url::parse(&origin).ok()?;
            if url.scheme() != "http"
                || url.host_str() != Some("127.0.0.1")
                || url.port().is_none()
                || !plain_url(&url)
            {
                return None;
            }
            let path = match self {
                Self::Stable => "stable.json",
                Self::Preview => "preview.json",
            };
            return Some((url.join(path).ok()?, key));
        }

        let update = app.config().plugins.0.get("updater")?;
        let key = update.get("pubkey")?.as_str()?.trim();
        let field = match self {
            Self::Stable => "stableEndpoint",
            Self::Preview => "previewEndpoint",
        };
        let url = Url::parse(update.get(field)?.as_str()?).ok()?;
        (url.scheme() == "https" && plain_url(&url) && !key.is_empty()).then(|| (url, key.into()))
    }
}

fn plain_url(url: &Url) -> bool {
    url.host().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
        && url.query().is_none()
}

#[derive(Default)]
pub(crate) struct UpdateState(Mutex<Option<Offer>>);

struct Offer {
    id: String,
    update: Update,
    installer: Option<Vec<u8>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateChannels {
    version: String,
    channels: Vec<UpdateChannel>,
    can_install: bool,
}

fn can_install() -> bool {
    !cfg!(debug_assertions)
        && matches!(
            tauri::utils::platform::bundle_type(),
            Some(tauri::utils::config::BundleType::Nsis)
        )
}

#[tauri::command]
pub(crate) async fn update_channels(app: AppHandle) -> AppResult<UpdateChannels> {
    Ok(UpdateChannels {
        version: app.package_info().version.to_string(),
        channels: [UpdateChannel::Stable, UpdateChannel::Preview]
            .into_iter()
            .filter(|channel| channel.endpoint(&app).is_some())
            .collect(),
        can_install: can_install(),
    })
}

#[derive(Serialize)]
pub(crate) struct UpdateInfo {
    id: String,
    version: String,
    notes: String,
}

#[tauri::command]
pub(crate) async fn check_update(
    app: AppHandle,
    state: State<'_, UpdateState>,
    channel: UpdateChannel,
) -> AppResult<Option<UpdateInfo>> {
    let mut offer = state.0.try_lock().map_err(|_| BUSY)?;
    *offer = None;
    let (endpoint, key) = channel
        .endpoint(&app)
        .ok_or("当前安装包未配置此更新渠道。")?;
    let local_rehearsal = endpoint.scheme() == "http";
    let updater = app
        .updater_builder()
        .pubkey(key)
        .endpoints(vec![endpoint.clone()])
        .map_err(|_| "更新渠道配置有误。")?
        .configure_client(move |client| {
            let client = client.redirect(Policy::none());
            if local_rehearsal {
                client.no_proxy()
            } else {
                client
            }
        })
        .timeout(Duration::from_secs(20))
        .version_comparator(move |current, remote| {
            remote.version.cmp_precedence(&current).is_gt()
                && (matches!(channel, UpdateChannel::Preview) || remote.version.pre.is_empty())
        })
        // This app guards its own tasks. Do not hide the app before Windows confirms it can launch the installer.
        .on_before_exit(|| {})
        .build()
        .map_err(|_| "无法初始化更新检查。")?;
    let Some(update) = updater
        .check()
        .await
        .map_err(|_| "无法读取更新信息，请检查网络后重试。")?
    else {
        return Ok(None);
    };
    if !plain_url(&update.download_url) || endpoint.origin() != update.download_url.origin() {
        return Err("更新包地址不在该渠道内，已停止更新。".into());
    }
    let info = UpdateInfo {
        id: new_id(),
        version: update.version.clone(),
        notes: update
            .body
            .as_deref()
            .unwrap_or_default()
            .chars()
            .take(3000)
            .collect(),
    };
    *offer = Some(Offer {
        id: info.id.clone(),
        update,
        installer: None,
    });
    Ok(Some(info))
}

#[derive(Serialize)]
pub(crate) struct DownloadProgress {
    bytes: usize,
    total: Option<u64>,
}

#[tauri::command]
pub(crate) async fn download_update(
    state: State<'_, UpdateState>,
    id: String,
    progress: Channel<DownloadProgress>,
) -> AppResult<()> {
    let mut slot = state.0.try_lock().map_err(|_| BUSY)?;
    let offer = slot
        .as_mut()
        .filter(|offer| offer.id == id)
        .ok_or(EXPIRED)?;
    offer.installer = None;
    let mut client = reqwest::Client::builder()
        .redirect(Policy::none())
        .timeout(Duration::from_secs(300));
    if offer.update.download_url.scheme() == "http" {
        client = client.no_proxy();
    }
    let download_error = "更新下载失败，请检查网络后重试。";
    let mut response = client
        .build()
        .map_err(|_| download_error)?
        .get(offer.update.download_url.clone())
        .send()
        .await
        .map_err(|_| download_error)?;
    if response.status().as_u16() != 200 {
        return Err(download_error.into());
    }
    let total = response.content_length();
    let too_large = "更新包超过大小限制，请联系维护者。";
    if total.is_some_and(|size| size > MAX_INSTALLER_BYTES as u64) {
        return Err(too_large.into());
    }
    let mut installer = Vec::new();
    let mut last_progress = 0;
    while let Some(chunk) = response.chunk().await.map_err(|_| download_error)? {
        if chunk.len() > MAX_INSTALLER_BYTES - installer.len() {
            return Err(too_large.into());
        }
        installer.extend_from_slice(&chunk);
        if installer.len() - last_progress >= 256 * 1024 {
            last_progress = installer.len();
            progress
                .send(DownloadProgress {
                    bytes: installer.len(),
                    total,
                })
                .map_err(|_| "下载已取消，请重新检查更新。")?;
        }
    }
    progress
        .send(DownloadProgress {
            bytes: installer.len(),
            total,
        })
        .map_err(|_| "下载已取消，请重新检查更新。")?;
    offer
        .update
        .verify(&installer)
        .map_err(|_| "更新签名或版本校验失败，请重新检查。")?;
    offer.installer = Some(installer);
    Ok(())
}

#[tauri::command]
pub(crate) async fn install_update(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, UpdateState>,
    id: String,
) -> AppResult<bool> {
    if !can_install() {
        return Err("请从正式 Windows 安装版启动 CortexOS 后再安装更新。".into());
    }
    let mut slot = state.0.try_lock().map_err(|_| BUSY)?;
    let offer = slot
        .as_mut()
        .filter(|offer| offer.id == id)
        .ok_or(EXPIRED)?;
    if offer.installer.is_none() {
        return Err("请先下载并校验更新包。".into());
    }
    let version = offer.update.version.clone();
    let confirm = app.clone();
    let agreed = tauri::async_runtime::spawn_blocking(move || {
        confirm
            .dialog()
            .message(format!(
                "安装 CortexOS {version} 会关闭应用。请先保存当前编辑内容并完成正在运行的任务。"
            ))
            .title("确认安装更新")
            .parent(&window)
            .buttons(MessageDialogButtons::OkCancelCustom(
                "安装更新".into(),
                "取消".into(),
            ))
            .blocking_show()
    })
    .await
    .map_err(|_| "无法打开更新确认窗口。")?;
    if !agreed {
        return Ok(false);
    }

    let source = app.state::<AppState>();
    let install_app = source.inner().clone();
    let update = offer.update.clone();
    // Windows' updater exits the process after a successful handoff. Keep SQLite locked until that handoff.
    // Move the already verified artifact into the worker and keep a single package in memory.
    let installer = offer.installer.take().ok_or(EXPIRED)?;
    let (installer, result) = tauri::async_runtime::spawn_blocking(move || {
        if install_app.closing.swap(true, Ordering::SeqCst) {
            return (installer, Err("应用正在关闭，请重新打开后再试。".into()));
        }
        let result = handoff(&install_app, &update, &installer);
        install_app.closing.store(false, Ordering::SeqCst);
        (installer, result)
    })
    .await
    .map_err(|_| "未能启动安装程序。")?;
    offer.installer = Some(installer);
    result
}

fn handoff(state: &AppState, update: &Update, installer: &[u8]) -> AppResult<bool> {
    use crate::benchmark::python::session::Kind;
    let db = state
        .database
        .as_ref()
        .map_err(|_| "本地数据库不可用，请先处理数据问题。")?;
    if state.has_shutdown_work()
        || state.close_cleanup.load(Ordering::SeqCst)
        || !db.pending_workspace_jobs()?.is_empty()
        || !db.pending_codex_runtime_jobs()?.is_empty()
        || !db.pending_claude_runtime_jobs()?.is_empty()
        || !db.pending_aider_runtime_jobs()?.is_empty()
        || !db.pending_sessions(Kind::Codex)?.is_empty()
        || !db.pending_sessions(Kind::Claude)?.is_empty()
        || !db.pending_sessions(Kind::Aider)?.is_empty()
    {
        return Err("仍有任务或项目副本等待处理。请完成或关闭相关任务，再安装更新。".into());
    }
    let _connection = db.connection()?;
    update
        .install(installer)
        .map_err(|_| "Windows 未能启动更新安装程序，请重试或手动下载安装包。")?;
    Ok(true)
}
