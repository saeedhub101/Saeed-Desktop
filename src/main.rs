#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

use saeed_desktop::ai::{AiProvider, LocalCommandAiProvider, OpenAiProvider};
use saeed_desktop::core::SaeedCore;
use saeed_desktop::storage::{AppSettings, Storage};
use saeed_desktop::voice::{MicrophoneRecorder, VoiceController};
use slint::SharedString;

slint::include_modules!();

const VOICE_ACTIVATION_THRESHOLD: f32 = 0.06;
const VOICE_SILENCE_TIMEOUT: Duration = Duration::from_millis(900);
const VOICE_MINIMUM_SPEECH: Duration = Duration::from_millis(120);

fn show_error_if_alive(weak: &slint::Weak<AppWindow>, message: &str) {
    let weak = weak.clone();
    let message = message.to_string();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(window) = weak.upgrade() {
            show_error(&window, message);
        }
    });
}

fn show_error(window: &AppWindow, message: String) {
    window.set_conversation(format!("Saeed: {message}").into());
    window.set_status("Error".into());
}

fn update_voice_ui(weak: &slint::Weak<AppWindow>, button: &str, status: &str) {
    let weak = weak.clone();
    let button = button.to_string();
    let status = status.to_string();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(window) = weak.upgrade() {
            window.set_voice_button_text(button.into());
            window.set_status(status.into());
        }
    });
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let window = AppWindow::new()?;
    let storage = Storage::open_default()?;
    let settings = storage.load_settings()?;
    let settings = Arc::new(Mutex::new(settings));
    window.set_settings_ai_model(settings.lock().unwrap().ai_model.clone().into());
    window.set_settings_stt_model(settings.lock().unwrap().stt_model.clone().into());
    window.set_settings_tts_model(settings.lock().unwrap().tts_model.clone().into());
    window.set_settings_tts_voice(settings.lock().unwrap().tts_voice.clone().into());
    if settings.lock().unwrap().openai_api_key.is_some() {
        window.set_settings_api_key_status("API key saved in Windows Credential Manager".into());
    }
    let persisted_messages = storage.load_messages().unwrap_or_default();
    let core = Arc::new(Mutex::new(SaeedCore::from_messages(persisted_messages.clone())));
    if let Some(last) = persisted_messages.last() {
        window.set_conversation(format!("{}: {}", last.role, last.content).into());
    }
    let voice_active = Arc::new(AtomicBool::new(false));
    let voice_worker_running = Arc::new(AtomicBool::new(false));

    let weak = window.as_weak();

    {
        let settings = Arc::clone(&settings);
        let weak = window.as_weak();
        window.on_save_settings(move |api_key, ai_model, stt_model, tts_model, tts_voice| {
            let mut next = {
                let Ok(current) = settings.lock() else {
                    if let Some(window) = weak.upgrade() {
                        window.set_settings_status("Settings state is unavailable.".into());
                    }
                    return;
                };
                current.clone()
            };

            let key = api_key.trim();
            if !key.is_empty() {
                next.openai_api_key = Some(key.to_string());
            }
            next.ai_model = ai_model.trim().to_string();
            next.stt_model = stt_model.trim().to_string();
            next.tts_model = tts_model.trim().to_string();
            next.tts_voice = tts_voice.trim().to_string();

            if next.ai_model.is_empty()
                || next.stt_model.is_empty()
                || next.tts_model.is_empty()
                || next.tts_voice.is_empty()
            {
                if let Some(window) = weak.upgrade() {
                    window.set_settings_status("All model/voice fields are required.".into());
                }
                return;
            }

            let Ok(storage) = Storage::open_default() else {
                if let Some(window) = weak.upgrade() {
                    window.set_settings_status("Could not open local settings storage.".into());
                }
                return;
            };

            if let Err(error) = storage.save_settings(&next) {
                if let Some(window) = weak.upgrade() {
                    window.set_settings_status(format!("Save failed: {error}").into());
                }
                return;
            }

            if let Ok(mut current) = settings.lock() {
                *current = next;
            }

            if let Some(window) = weak.upgrade() {
                window.set_settings_status("Saved securely. API key is stored in Windows Credential Manager.".into());
                window.set_settings_open(false);
                window.set_status("Ready • Settings saved".into());
            }
        });
    }

    {
        let settings = Arc::clone(&settings);
        let weak = window.as_weak();
        window.on_clear_api_key(move || {
            let Ok(storage) = Storage::open_default() else {
                if let Some(window) = weak.upgrade() {
                    window.set_settings_status("Could not open local settings storage.".into());
                }
                return;
            };

            if let Err(error) = storage.clear_openai_api_key() {
                if let Some(window) = weak.upgrade() {
                    window.set_settings_status(format!("Could not clear API key: {error}").into());
                }
                return;
            }

            if let Ok(mut current) = settings.lock() {
                current.openai_api_key = None;
            }

            if let Some(window) = weak.upgrade() {
                window.set_settings_api_key_status("No API key saved".into());
                window.set_settings_status("API key removed from Windows Credential Manager.".into());
            }
        });
    }

    {
        let settings = Arc::clone(&settings);
        let core = Arc::clone(&core);
        let weak = window.as_weak();

        window.on_send_message(move |text: SharedString| {
            let text = text.trim().to_string();
            if text.is_empty() {
                return;
            }

            let settings = {
                let Ok(settings) = settings.lock() else {
                    show_error_if_alive(&weak, "Settings state is unavailable.");
                    return;
                };
                settings.clone()
            };

            let core = Arc::clone(&core);
            let weak = weak.clone();

            let _ = slint::invoke_from_event_loop({
                let weak = weak.clone();
                move || {
                    if let Some(window) = weak.upgrade() {
                        window.set_status("Thinking…".into());
                    }
                }
            });

            thread::spawn(move || {
                let result = (|| -> Result<String, String> {
                    let mut core = core.lock()
                        .map_err(|_| "Core state is unavailable.".to_string())?;

                    core.add_chat_message("user", &text);
                    if let Some(message) = core.messages().last() {
                        if let Ok(storage) = Storage::open_default() {
                            storage.append_message(message)
                                .map_err(|error| format!("Could not persist message: {error}"))?;
                        }
                    }

                    let response = if settings.ai_model.trim().eq_ignore_ascii_case("local") {
                        let provider = LocalCommandAiProvider::from_environment()?;
                        core.complete_chat(&provider)?
                    } else {
                        let api_key = settings.openai_api_key.clone()
                            .ok_or_else(|| "Add your OpenAI API key in Settings first.".to_string())?;
                        let provider = OpenAiProvider::from_config(api_key, settings.ai_model)?;
                        core.complete_chat(&provider)?
                    };

                    if let Some(message) = core.messages().last() {
                        if let Ok(storage) = Storage::open_default() {
                            storage.append_message(message)
                                .map_err(|error| format!("Could not persist response: {error}"))?;
                        }
                    }

                    Ok(response.text)
                })();

                let weak = weak.clone();
                let text = text.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(window) = weak.upgrade() {
                        match result {
                            Ok(response) => {
                                window.set_conversation(
                                    format!("You: {text}\n\nSaeed: {response}").into(),
                                );
                                window.set_status("Ready • Shared conversation".into());
                            }
                            Err(error) => show_error(window.as_ref(), error),
                        }
                    }
                });
            });
        });
    }

    {
        let active = Arc::clone(&voice_active);
        let worker_running = Arc::clone(&voice_worker_running);
        let core = Arc::clone(&core);
        let settings = Arc::clone(&settings);
        let weak_for_handler = weak.clone();

        window.on_toggle_voice(move || {
            let Some(window) = weak_for_handler.upgrade() else {
                return;
            };

            if active.load(Ordering::Acquire) {
                active.store(false, Ordering::Release);
                window.set_voice_button_text("Mic ON".into());
                window.set_status("Microphone off".into());
                return;
            }

            active.store(true, Ordering::Release);
            window.set_voice_button_text("Mic OFF".into());
            window.set_status("Listening…".into());

            if worker_running.swap(true, Ordering::AcqRel) {
                return;
            }

            let active = Arc::clone(&active);
            let worker_running = Arc::clone(&worker_running);
            let core = Arc::clone(&core);
            let settings = Arc::clone(&settings);
            let weak = weak_for_handler.clone();

            thread::spawn(move || {
                let mut recorder = MicrophoneRecorder::new();

                while active.load(Ordering::Acquire) {
                    if let Err(error) = recorder.start() {
                        update_voice_ui(&weak, "Mic ON", &format!("Microphone unavailable: {error}"));
                        active.store(false, Ordering::Release);
                        break;
                    }

                    update_voice_ui(&weak, "Mic OFF", "Listening…");

                    let audio = match recorder.capture_utterance(
                        &active,
                        VOICE_ACTIVATION_THRESHOLD,
                        VOICE_SILENCE_TIMEOUT,
                        VOICE_MINIMUM_SPEECH,
                    ) {
                        Ok(Some(audio)) => audio,
                        Ok(None) => break,
                        Err(error) => {
                            update_voice_ui(&weak, "Mic ON", &format!("Microphone error: {error}"));
                            active.store(false, Ordering::Release);
                            break;
                        }
                    };

                    if !active.load(Ordering::Acquire) {
                        break;
                    }

                    update_voice_ui(&weak, "Mic OFF", "Transcribing…");

                    match VoiceController::process_turn(&core, &settings, &audio) {
                        Ok(result) => {
                            let transcript = result.transcript;
                            let response = result.response;
                            if let Ok(core) = core.lock() {
                                if let Ok(storage) = Storage::open_default() {
                                    for message in core.messages().iter().rev().take(2).rev() {
                                        let _ = storage.append_message(message);
                                    }
                                }
                            }
                            let transcript_for_ui = transcript.clone();
                            let response_for_ui = response.clone();
                            let weak_for_ui = weak.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(window) = weak_for_ui.upgrade() {
                                    window.set_conversation(
                                        format!(
                                            "You (Voice): {transcript_for_ui}\n\nSaeed: {response_for_ui}"
                                        )
                                        .into(),
                                    );
                                    window.set_status("Speaking…".into());
                                }
                            });

                            if let Err(error) = VoiceController::speak(&settings, &response) {
                                update_voice_ui(&weak, "Mic ON", &format!("TTS unavailable: {error}"));
                                active.store(false, Ordering::Release);
                                break;
                            }

                            if active.load(Ordering::Acquire) {
                                update_voice_ui(&weak, "Mic OFF", "Listening…");
                            }
                        }
                        Err(error) => {
                            update_voice_ui(&weak, "Mic ON", &format!("Voice error: {error}"));
                            active.store(false, Ordering::Release);
                            break;
                        }
                    }
                }

                worker_running.store(false, Ordering::Release);

                if active.load(Ordering::Acquire) {
                    active.store(false, Ordering::Release);
                }

                update_voice_ui(&weak, "Mic ON", "Ready • Shared conversation");
            });
        });
    }

    window.run()?;
    Ok(())
}

