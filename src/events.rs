use tao::{
    event::{ElementState, KeyEvent},
    keyboard::{KeyCode, ModifiersState},
};

use crate::history::{IpcMessage, NavCommand};

#[derive(Debug)]
pub enum ShellEvent {
    Menu(muda::MenuEvent),
    Preview(PreviewEvent),
}

#[derive(Debug)]
pub enum PreviewEvent {
    Command(NavCommand),
    Reveal(String),
    CopySource(String),
    LiveReload(bool),
    Devtools(bool),
    InspectorPrefs(String),
    Layout(String),
    Location(String),
    Drag,
    Zoom,
    Loaded(String),
    Title(String),
    PickFolder,
    PickFolderResult(Option<String>),
    Navigate {
        url: String,
        title: String,
        inspector_url: Option<String>,
    },
    Evaluate(String),
}

pub trait PreviewSink: Send + Sync {
    fn send(&self, event: PreviewEvent);
}

impl From<IpcMessage> for PreviewEvent {
    fn from(message: IpcMessage) -> Self {
        match message {
            IpcMessage::Nav(command) => Self::Command(command),
            IpcMessage::Reveal(path) => Self::Reveal(path),
            IpcMessage::CopySource(path) => Self::CopySource(path),
            IpcMessage::LiveReload(enabled) => Self::LiveReload(enabled),
            IpcMessage::Devtools(open) => Self::Devtools(open),
            IpcMessage::InspectorPrefs(json) => Self::InspectorPrefs(json),
            IpcMessage::Layout(json) => Self::Layout(json),
            IpcMessage::Location(url) => Self::Location(url),
            IpcMessage::Drag => Self::Drag,
            IpcMessage::Zoom => Self::Zoom,
            IpcMessage::PickFolder => Self::PickFolder,
        }
    }
}

pub fn is_close_shortcut(key: KeyCode, modifiers: ModifiersState) -> bool {
    key == KeyCode::KeyW && close_modifier(modifiers)
}

pub fn is_close_key_event(event: &KeyEvent, modifiers: ModifiersState) -> bool {
    event.state == ElementState::Pressed
        && !event.repeat
        && is_close_shortcut(event.physical_key, modifiers)
}

fn close_modifier(modifiers: ModifiersState) -> bool {
    #[cfg(target_os = "macos")]
    {
        modifiers.super_key()
            && !modifiers.control_key()
            && !modifiers.alt_key()
            && !modifiers.shift_key()
    }
    #[cfg(not(target_os = "macos"))]
    {
        modifiers.control_key()
            && !modifiers.super_key()
            && !modifiers.alt_key()
            && !modifiers.shift_key()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_shortcut_uses_platform_modifier() {
        #[cfg(target_os = "macos")]
        {
            assert!(is_close_shortcut(KeyCode::KeyW, ModifiersState::SUPER));
            assert!(!is_close_shortcut(KeyCode::KeyW, ModifiersState::CONTROL));
            assert!(!is_close_shortcut(
                KeyCode::KeyW,
                ModifiersState::SUPER | ModifiersState::SHIFT
            ));
            assert!(!is_close_shortcut(
                KeyCode::KeyW,
                ModifiersState::SUPER | ModifiersState::CONTROL
            ));
        }
        #[cfg(not(target_os = "macos"))]
        {
            assert!(is_close_shortcut(KeyCode::KeyW, ModifiersState::CONTROL));
            assert!(!is_close_shortcut(KeyCode::KeyW, ModifiersState::SUPER));
            assert!(!is_close_shortcut(
                KeyCode::KeyW,
                ModifiersState::CONTROL | ModifiersState::SHIFT
            ));
            assert!(!is_close_shortcut(
                KeyCode::KeyW,
                ModifiersState::CONTROL | ModifiersState::ALT
            ));
        }
        assert!(!is_close_shortcut(KeyCode::KeyN, ModifiersState::SUPER));
        assert!(!is_close_shortcut(KeyCode::KeyW, ModifiersState::empty()));
    }
}
