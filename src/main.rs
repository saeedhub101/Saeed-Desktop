#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::cell::RefCell;
use std::rc::Rc;

use saeed_desktop::ai::OpenAiProvider;
use saeed_desktop::core::SaeedCore;
use slint::SharedString;

slint::include_modules!();

fn show_completion_error(window: &AppWindow, error: String) {
    window.set_conversation(format!("Saeed: {error}").into());
    window.set_status("AI unavailable".into());
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let window = AppWindow::new()?;
    let core = Rc::new(RefCell::new(SaeedCore::new()));

    let weak = window.as_weak();
    let core_for_chat = Rc::clone(&core);

    window.on_send_message(move |text: SharedString| {
        let text = text.trim();
        if text.is_empty() {
            return;
        }

        let Some(window) = weak.upgrade() else {
            return;
        };

        core_for_chat
            .borrow_mut()
            .add_chat_message("user", text);

        window.set_status("Thinking…".into());

        match OpenAiProvider::from_environment() {
            Ok(provider) => {
                let result = core_for_chat.borrow_mut().complete_chat(&provider);
                match result {
                    Ok(response) => {
                        window.set_conversation(
                            format!("You: {text}\n\nSaeed: {}", response.text).into(),
                        );
                        window.set_status("Ready • Shared conversation".into());
                    }
                    Err(error) => show_completion_error(&window, error),
                }
            }
            Err(error) => show_completion_error(&window, error),
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

            let Some(window) = weak.upgrade() else {
                return;
            };

            core.borrow_mut().add_voice_message("user", text);
            window.set_status("Thinking… • Voice".into());

            match OpenAiProvider::from_environment() {
                Ok(provider) => {
                    let result = core.borrow_mut().complete_voice(&provider);
                    match result {
                        Ok(response) => {
                            window.set_conversation(
                                format!("You (Voice): {text}\n\nSaeed: {}", response.text)
                                    .into(),
                            );
                            window.set_status("Ready • Shared conversation".into());
                        }
                        Err(error) => show_completion_error(&window, error),
                    }
                }
                Err(error) => show_completion_error(&window, error),
            }
        }
    });

    window.run()?;
    Ok(())
}
