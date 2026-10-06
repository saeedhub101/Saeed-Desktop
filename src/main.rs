#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::cell::RefCell;
use std::rc::Rc;

use saeed_desktop::core::SaeedCore;
use slint::SharedString;

slint::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let window = AppWindow::new()?;
    let core = Rc::new(RefCell::new(SaeedCore::new()));
    let weak = window.as_weak();
    let core_for_ui = Rc::clone(&core);

    window.on_send_message(move |text: SharedString| {
        let text = text.trim();
        if text.is_empty() {
            return;
        }

        core_for_ui.borrow_mut().add_chat_message("user", text);

        if let Some(window) = weak.upgrade() {
            window.set_conversation(
                format!("You: {text}\n\nSaeed: Session received. AI provider will be connected next.")
                    .into(),
            );
            window.set_status("Shared conversation • Chat input".into());
        }
    });

    window.on_submit_voice_transcript({
        let weak = window.as_weak();
        let core = Rc::clone(&core);
        move |text: SharedString| {
            let text = text.trim();
            if text.is_empty() {
                return;
            }

            core.borrow_mut().add_voice_message("user", text);

            if let Some(window) = weak.upgrade() {
                window.set_conversation(
                    format!("You (Voice): {text}\n\nSaeed: Session received. Continue by voice or Chat.")
                        .into(),
                );
                window.set_status("Shared conversation • Voice input".into());
            }
        }
    });

    window.run()?;
    Ok(())
}
