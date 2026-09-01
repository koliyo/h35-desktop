use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{Result, WindowConfig, WindowId};
use tao::{
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
    keyboard::ModifiersState,
    platform::run_return::EventLoopExtRunReturn,
};
use wry::{PageLoadEvent, WebContext};

use crate::{
    chrome,
    events::{self, PreviewEvent, PreviewSink, ShellEvent},
    history::{self, IpcMessage, NavCommand, NavHistory},
    menu::{self, MenuConfig},
    source, web_context_dir,
    window::{LiveWindow, WebViewHooks, WindowCreate},
};

pub type IpcHandler = Arc<dyn Fn(&str, Arc<dyn PreviewSink>) + Send + Sync>;
pub type NavigateHandler = Arc<dyn Fn(&str) + Send + Sync>;

pub struct HostOptions {
    pub title: String,
    pub identifier: String,
    pub state_dir: PathBuf,
    pub url: String,
    pub home_url: Option<String>,
    pub icon_png: Option<&'static [u8]>,
    pub live_reload: bool,
    pub source_root: Option<PathBuf>,
    pub inspector_url: Option<String>,
    pub picker: bool,
    pub goto: bool,
    pub find: bool,
    pub width: f64,
    pub height: f64,
    pub devtools: bool,
    pub extra_initialization_script: Option<String>,
    pub on_ipc: Option<IpcHandler>,
    pub on_navigate: Option<NavigateHandler>,
    pub check_updates: bool,
}

impl Default for HostOptions {
    fn default() -> Self {
        let defaults = WindowConfig::default();
        Self {
            title: defaults.title,
            identifier: "dev.h35.preview".into(),
            state_dir: PathBuf::from("."),
            url: String::new(),
            home_url: None,
            icon_png: None,
            live_reload: true,
            source_root: None,
            inspector_url: None,
            picker: false,
            goto: true,
            find: true,
            width: defaults.width,
            height: defaults.height,
            devtools: true,
            extra_initialization_script: None,
            on_ipc: None,
            on_navigate: None,
            check_updates: false,
        }
    }
}

struct ProxySink(EventLoopProxy<ShellEvent>);

impl PreviewSink for ProxySink {
    fn send(&self, event: PreviewEvent) {
        send_preview(&self.0, event);
    }
}

fn send_preview(proxy: &EventLoopProxy<ShellEvent>, event: PreviewEvent) {
    let _ = proxy.send_event(ShellEvent::Preview(event));
}

pub fn preview(options: HostOptions) -> Result<()> {
    let mut event_loop = EventLoopBuilder::<ShellEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let id = WindowId::new("preview");
    let state_key = format!("{}:preview", options.identifier);
    let state_dir = options.state_dir.clone();

    let saved_state = crate::state::load_window_state(&state_dir, &state_key);
    let (width, height) = match &saved_state {
        Some(state) if state.width >= 100.0 && state.height >= 100.0 => (state.width, state.height),
        _ => (options.width, options.height),
    };

    let (initial_position, initial_maximized) = match &saved_state {
        Some(state) => {
            let visible =
                crate::state::is_position_visible(&event_loop, state.x, state.y, width, height);
            let pos = if visible {
                Some(state.position())
            } else {
                None
            };
            (pos, state.is_maximized)
        }
        None => (None, false),
    };

    let template = WindowConfig {
        title: options.title.clone(),
        width,
        height,
        ..WindowConfig::default()
    };
    let context = WebContext::new(Some(web_context_dir(&options.identifier, &id)));
    let load_proxy = proxy.clone();
    let title_proxy = proxy.clone();
    let mut inspector_url = options.inspector_url.clone();
    let saved_inspector = crate::state::load_inspector_state(&state_dir, &state_key);
    let saved_layout = saved_state
        .as_ref()
        .filter(|state| state.has_layout())
        .cloned();
    let live = LiveWindow::create(
        &event_loop,
        WindowCreate {
            template,
            id,
            url: options.url.clone(),
            context,
            devtools: options.devtools,
            hooks: WebViewHooks {
                initialization_script: Some(build_init_script(
                    &options,
                    saved_inspector.as_ref(),
                    saved_layout.as_ref(),
                )),
                ipc_handler: Some(ipc_handler(
                    proxy.clone(),
                    options.on_ipc.clone(),
                    Arc::new(ProxySink(proxy.clone())),
                )),
                on_page_load: Some(Box::new(move |event, url| {
                    if matches!(event, PageLoadEvent::Finished) {
                        send_preview(&load_proxy, PreviewEvent::Loaded(url));
                    }
                })),
                on_title_changed: Some(Box::new(move |title| {
                    send_preview(&title_proxy, PreviewEvent::Title(title));
                })),
            },
            position: initial_position,
            maximized: initial_maximized,
            unified_titlebar: cfg!(target_os = "macos"),
            icon_png: options.icon_png,
        },
    )?;
    let pick_proxy = proxy.clone();
    let bundled = std::env::current_exe()
        .ok()
        .is_some_and(|exe| crate::bundle::running_inside_app_bundle(&exe));
    let updater = crate::update::start(
        options.check_updates && crate::update::updater_allowed(bundled, true),
    );
    let menu = menu::NativeMenu::install(
        proxy,
        MenuConfig {
            app_name: &options.title,
            version: None,
            new_window: false,
            navigation: true,
            search: options.find,
            goto: options.goto,
            reload: true,
            live_reload_on: options.live_reload,
            devtools: options.devtools,
            picker: options.picker,
            check_updates: options.check_updates,
            check_updates_enabled: updater.is_some(),
        },
    )?;
    menu.attach(&live.window)?;

    let mut history = NavHistory::new(options.home_url.as_deref().unwrap_or(&options.url));
    let on_navigate = options.on_navigate.clone();
    let mut title = options.title;
    let mut modifiers = ModifiersState::empty();
    let source_root = options.source_root.clone();
    let icon_png = options.icon_png;
    let devtools_enabled = options.devtools;
    let mut last_persist = Instant::now() - Duration::from_secs(1);
    event_loop.run_return(|event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        let _keep = &menu;
        handle_event(
            &mut PreviewSession {
                live: &live,
                menu: &menu,
                history: &mut history,
                title: &mut title,
                modifiers: &mut modifiers,
                last_persist: &mut last_persist,
                inspector_url: &mut inspector_url,
                state_dir: &state_dir,
                state_key: &state_key,
                source_root: source_root.as_deref(),
                on_navigate: &on_navigate,
                updater: &updater,
                pick_proxy: &pick_proxy,
                icon_png,
                devtools_enabled,
            },
            event,
            control_flow,
        );
    });

    crate::state::persist_window_state(&state_dir, &state_key, &live.window);
    Ok(())
}

fn build_init_script(
    options: &HostOptions,
    saved_inspector: Option<&crate::state::InspectorState>,
    saved_layout: Option<&crate::state::WindowState>,
) -> String {
    let mut init_script = chrome::initialization_script(
        options.inspector_url.as_deref(),
        options.source_root.is_some(),
        options.live_reload,
        options.goto,
        options.find,
        saved_inspector,
        saved_layout,
    );
    if let Some(extra) = &options.extra_initialization_script {
        init_script.push('\n');
        init_script.push_str(extra);
    }
    init_script
}

fn ipc_handler(
    proxy: EventLoopProxy<ShellEvent>,
    host_ipc: Option<IpcHandler>,
    host_sink: Arc<dyn PreviewSink>,
) -> Box<dyn Fn(wry::http::Request<String>) + 'static> {
    Box::new(move |request| match IpcMessage::parse(request.body()) {
        Some(IpcMessage::Drag) => {
            #[cfg(target_os = "macos")]
            crate::window::begin_toolbar_drag();
            #[cfg(not(target_os = "macos"))]
            send_preview(&proxy, PreviewEvent::Drag);
        }
        Some(message) => send_preview(&proxy, message.into()),
        None => {
            if let Some(handler) = &host_ipc {
                handler(request.body(), host_sink.clone());
            }
        }
    })
}

struct PreviewSession<'a> {
    live: &'a LiveWindow,
    menu: &'a menu::NativeMenu,
    history: &'a mut NavHistory,
    title: &'a mut String,
    modifiers: &'a mut ModifiersState,
    last_persist: &'a mut Instant,
    inspector_url: &'a mut Option<String>,
    state_dir: &'a Path,
    state_key: &'a str,
    source_root: Option<&'a Path>,
    on_navigate: &'a Option<NavigateHandler>,
    updater: &'a Option<crate::update::Handle>,
    pick_proxy: &'a EventLoopProxy<ShellEvent>,
    icon_png: Option<&'static [u8]>,
    devtools_enabled: bool,
}

fn handle_event(
    session: &mut PreviewSession<'_>,
    event: Event<'_, ShellEvent>,
    control_flow: &mut ControlFlow,
) {
    match event {
        Event::NewEvents(StartCause::Init) => {
            crate::icon::apply_host_icon(session.icon_png);
            session.live.realize_unified_chrome();
        }
        Event::RedrawEventsCleared => session.live.sync_unified_chrome(),
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => persist_and_exit(session, control_flow),
        Event::WindowEvent {
            event: WindowEvent::Moved(_) | WindowEvent::Resized(_),
            ..
        } => {
            session.live.sync_unified_chrome();
            if session.last_persist.elapsed() >= Duration::from_millis(250) {
                crate::state::persist_window_state(
                    session.state_dir,
                    session.state_key,
                    &session.live.window,
                );
                *session.last_persist = Instant::now();
            }
        }
        Event::WindowEvent {
            event: WindowEvent::ModifiersChanged(next),
            ..
        } => *session.modifiers = next,
        Event::WindowEvent {
            event: WindowEvent::KeyboardInput { event, .. },
            ..
        } if events::is_close_key_event(&event, *session.modifiers) => {
            persist_and_exit(session, control_flow);
        }
        Event::UserEvent(ShellEvent::Menu(menu_event)) => {
            handle_menu_event(session, &menu_event, control_flow);
        }
        Event::UserEvent(ShellEvent::Preview(preview_event)) => {
            handle_preview_event(session, preview_event);
        }
        _ => {}
    }
}

fn persist_and_exit(session: &PreviewSession<'_>, control_flow: &mut ControlFlow) {
    crate::state::persist_window_state(session.state_dir, session.state_key, &session.live.window);
    *control_flow = ControlFlow::Exit;
}

fn menu_action(event: &muda::MenuEvent) -> Option<&'static str> {
    const IDS: &[&str] = &[
        menu::CLOSE_WINDOW_ID,
        menu::QUIT_ID,
        menu::BACK_ID,
        menu::FORWARD_ID,
        menu::HOME_ID,
        menu::RELOAD_ID,
        menu::LIVE_RELOAD_ID,
        menu::WEB_INSPECTOR_ID,
        menu::FIND_ID,
        menu::FIND_NEXT_ID,
        menu::FIND_PREVIOUS_ID,
        menu::USE_SELECTION_ID,
        menu::GO_TO_FILE_ID,
        menu::OPEN_PICKER_ID,
        menu::SELECT_ALL_ID,
        menu::CHECK_UPDATES_ID,
    ];
    IDS.iter().copied().find(|id| menu::is(event, id))
}

fn handle_menu_event(
    session: &mut PreviewSession<'_>,
    menu_event: &muda::MenuEvent,
    control_flow: &mut ControlFlow,
) {
    match menu_action(menu_event) {
        Some(menu::CLOSE_WINDOW_ID | menu::QUIT_ID) => persist_and_exit(session, control_flow),
        Some(menu::BACK_ID) => apply_command(session.live, session.history, NavCommand::Back),
        Some(menu::FORWARD_ID) => apply_command(session.live, session.history, NavCommand::Forward),
        Some(menu::HOME_ID) => apply_command(session.live, session.history, NavCommand::Home),
        Some(menu::RELOAD_ID) => apply_command(session.live, session.history, NavCommand::Reload),
        Some(menu::LIVE_RELOAD_ID) => apply_overlay(
            session.live,
            &chrome::live_reload_set_script(session.menu.live_reload_checked()),
        ),
        Some(menu::WEB_INSPECTOR_ID) => {
            if session.live.webview.is_devtools_open() {
                session.live.webview.close_devtools();
            } else {
                session.live.webview.open_devtools();
            }
        }
        Some(menu::FIND_ID) => apply_overlay(session.live, chrome::FIND_OPEN_SCRIPT),
        Some(menu::FIND_NEXT_ID) => apply_overlay(session.live, chrome::FIND_NEXT_SCRIPT),
        Some(menu::FIND_PREVIOUS_ID) => apply_overlay(session.live, chrome::FIND_PREV_SCRIPT),
        Some(menu::USE_SELECTION_ID) => {
            apply_overlay(session.live, chrome::FIND_USE_SELECTION_SCRIPT)
        }
        Some(menu::GO_TO_FILE_ID) => apply_overlay(session.live, chrome::GOTO_OPEN_SCRIPT),
        Some(menu::OPEN_PICKER_ID) => apply_overlay(session.live, chrome::PICKER_OPEN_SCRIPT),
        Some(menu::SELECT_ALL_ID) => apply_overlay(session.live, chrome::SELECT_ALL_SCRIPT),
        Some(menu::CHECK_UPDATES_ID) => {
            if let Some(updater) = session.updater.as_ref() {
                updater.check_for_updates();
            }
        }
        _ => {}
    }
}

fn handle_preview_event(session: &mut PreviewSession<'_>, event: PreviewEvent) {
    match event {
        PreviewEvent::Command(command) => {
            apply_command(session.live, session.history, command);
        }
        PreviewEvent::Reveal(spec) => {
            apply_source(session.source_root, &spec, SourceAction::Reveal);
        }
        PreviewEvent::CopySource(spec) => {
            apply_source(session.source_root, &spec, SourceAction::Copy);
        }
        PreviewEvent::LiveReload(enabled) => {
            session.menu.set_live_reload_checked(enabled);
        }
        PreviewEvent::Devtools(open) => {
            if !session.devtools_enabled {
                return;
            }
            if open {
                if !session.live.webview.is_devtools_open() {
                    session.live.webview.open_devtools();
                }
            } else if session.live.webview.is_devtools_open() {
                session.live.webview.close_devtools();
            }
        }
        PreviewEvent::InspectorPrefs(json) => {
            if let Some(state) = crate::state::parse_inspector_state_json(&json) {
                crate::state::save_inspector_state(session.state_dir, session.state_key, state);
            }
        }
        PreviewEvent::Layout(json) => {
            crate::state::merge_layout_json(session.state_dir, session.state_key, &json);
        }
        PreviewEvent::Drag => {
            if let Err(error) = session.live.window.drag_window() {
                tracing::error!(%error, "failed to drag preview window");
            }
        }
        PreviewEvent::Zoom => {
            session
                .live
                .window
                .set_maximized(!session.live.window.is_maximized());
        }
        PreviewEvent::PickFolder => {
            crate::dialog::start_pick_folder(session.pick_proxy.clone());
        }
        PreviewEvent::PickFolderResult(path) => {
            apply_overlay(
                session.live,
                &crate::dialog::pick_folder_result_script(path.as_deref()),
            );
        }
        PreviewEvent::Loaded(url) => {
            if history::is_inspector_document(&url, session.inspector_url.as_deref()) {
                return;
            }
            commit_navigation(session, &url);
            session.live.sync_unified_chrome();
        }
        PreviewEvent::Title(next) => {
            session.live.window.set_title(&next);
            *session.title = next;
            apply_overlay(session.live, &chrome::update_title_script(session.title));
        }
        PreviewEvent::Location(url) => {
            commit_navigation(session, &url);
        }
        PreviewEvent::Navigate {
            url,
            title: next_title,
            inspector_url: next_inspector,
        } => {
            session.history.reset_origin(&url);
            if let Err(error) = session.live.webview.load_url(&url) {
                tracing::error!(%error, "failed to load preview origin");
            }
            session.live.window.set_title(&next_title);
            *session.title = next_title;
            if let Some(inspector) = next_inspector {
                *session.inspector_url = Some(inspector.clone());
                apply_overlay(session.live, &chrome::set_inspector_script(&inspector));
            }
        }
        PreviewEvent::Evaluate(script) => {
            apply_overlay(session.live, &script);
        }
    }
}

fn commit_navigation(session: &mut PreviewSession<'_>, url: &str) {
    session.history.commit(url);
    if let Some(on_navigate) = session.on_navigate {
        on_navigate(url);
    }
    sync_chrome(session.live, session.history, session.title);
}

fn apply_command(live: &LiveWindow, history: &mut NavHistory, command: NavCommand) {
    let result = match command {
        NavCommand::Back => {
            if history.request_back() {
                live.webview.evaluate_script("history.back()")
            } else {
                return;
            }
        }
        NavCommand::Forward => {
            if history.request_forward() {
                live.webview.evaluate_script("history.forward()")
            } else {
                return;
            }
        }
        NavCommand::Home => {
            history.request_home();
            live.webview.load_url(history.home())
        }
        NavCommand::Reload => live.webview.reload(),
    };
    if let Err(error) = result {
        tracing::error!(%error, ?command, "failed to apply preview navigation");
    }
}

fn apply_overlay(live: &LiveWindow, script: &str) {
    if let Err(error) = live.webview.evaluate_script(script) {
        tracing::error!(%error, "failed to apply preview overlay action");
    }
}

enum SourceAction {
    Reveal,
    Copy,
}

fn apply_source(root: Option<&Path>, spec: &str, action: SourceAction) {
    let Some(root) = root else {
        return;
    };
    let Some(path) = source::resolve_source_file(root, spec) else {
        tracing::warn!(spec, root = %root.display(), "preview source file not found");
        return;
    };
    match action {
        SourceAction::Reveal => source::reveal_in_file_manager(&path),
        SourceAction::Copy => source::copy_file_text(&path),
    }
}

fn sync_chrome(live: &LiveWindow, history: &NavHistory, title: &str) {
    if let Err(error) = live.webview.evaluate_script(&chrome::update_script(
        title,
        &history.display_path(),
        history.can_back(),
        history.can_forward(),
    )) {
        tracing::error!(%error, "failed to update preview chrome");
    }
}
