#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use saeed_desktop::ai::OpenAiProvider;
use saeed_desktop::core::SaeedCore;
use saeed_desktop::voice::{
    AudioPlayer, MicrophoneRecorder, OpenAiSpeechToText, OpenAiTextToSpeech, SpeechToText,
    TextToSpeech,
};
use slint::SharedString;

slint::include_modules!();

fn show_error(window: &AppWindow, message: String) {
    window.set_conversation(format!("Saeed: {message}").into());
    window.set_status("Voice unavailable".into());
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let window = AppWindow::new()?;
    let core = Arc::new(Mutex::new(SaeedCore::new()));
    let recorder = Rc::new(RefCell::new(MicrophoneRecorder::new()));

    let weak = window.as_weak();
    let core_for_chat = Arc::clone(&core);

    window.on_send_message(move |text: SharedString| {
        let text = text.trim();
        if text.is_empty() {
            return;
        }

        let Some(window) = weak.upgrade() else {
            return;
        };

        let Ok(mut core) = core_for_chat.lock() else {
            show_error(&window, "Core state is unavailable.".to_string());
            return;
        };

        core.add_chat_message("user", text);
        window.set_status("Thinking…".into());

        match OpenAiProvider::from_environment() {
            Ok(provider) => match core.complete_chat(&provider) {
                Ok(response) => {
                    window.set_conversation(
                        format!("You: {text}\n\nSaeed: {}", response.text).into(),
                    );
                    window.set_status("Ready • Shared conversation".into());
                }
                Err(error) => show_error(&window, error),
            },
            Err(error) => show_error(&window, error),
        }
    });

    {
        let recorder = Rc::clone(&recorder);
        let core = Arc::clone(&core);
        let weak = window.as_weak();

        window.on_toggle_voice(move || {
            let Some(window) = weak.upgrade() else {
                return;
            };

            let is_recording = recorder.borrow().is_recording();

            if !is_recording {
                match recorder.borrow_mut().start() {
                    Ok(()) => {
                        window.set_voice_button_text("Stop");
                        window.set_status("Listening…".into());
                    }
                    Err(error) => show_error(&window, error),
                }
                return;
            }

            let audio = match recorder.borrow_mut().stop() {
                Ok(audio) => audio,
                Err(error) => {
                    window.set_voice_button_text("Voice");
                    show_error(&window, error);
                    return;
                }
            };

            window.set_voice_button_text("Voice");
            window.set_status("Transcribing…".into());

            let core = Arc::clone(&core);
            let weak = window.as_weak();

            std::thread::spawn(move || {
                let result = process_voice_turn(&core, &audio);

                let _ = slint::invoke_from_event_loop(move || {
                    let Some(window) = weak.upgrade() else {
                        return;
                    };

                    match result {
                        Ok((transcript, response)) => {
                            window.set_conversation(
                                format!(
                                    "You (Voice): {transcript}\n\nSaeed: {}",
                                    response
                                )
                                .into(),
                            );
                            window.set_status("Ready • Shared conversation".into());
                        }
                        Err(error) => show_error(&window, error),
                    }
                });
            });
        });
    }

    window.run()?;
    Ok(())
}

fn process_voice_turn(
    core: &Arc<Mutex<SaeedCore>>,
    audio: &[u8],
) -> Result<(String, String), String> {
    let stt = OpenAiSpeechToText::from_environment()?;
    let transcript = stt.transcribe(audio)?;

    let response = {
        let mut core = core
            .lock()
            .map_err(|_| "Core state is unavailable.".to_string())?;

        core.add_voice_message("user", &transcript);

        let ai = OpenAiProvider::from_environment()?;
        core.complete_voice(&ai)?.text
    };

    let tts = OpenAiTextToSpeech::from_environment()?;
    let spoken_audio = tts.synthesize(&response)?;
    AudioPlayer::play(&spoken_audio)?;

    Ok((transcript, response))
}
