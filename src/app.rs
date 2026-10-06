use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

use saeed_desktop::ai::{AiProvider, AiRequest, LocalCommandAiProvider, OpenAiProvider, OpenAiCompatibleProvider};
use saeed_desktop::core::SaeedCore;
use saeed_desktop::character::{CharacterRuntime, CharacterState, CharacterVisibility};
use saeed_desktop::storage::Storage;
use saeed_desktop::voice::{MicrophoneRecorder, VoiceController};
use slint::SharedString;
slint::include_modules!();



const VOICE_ACTIVATION_THRESHOLD: f32 = 0.06;
const VOICE_SILENCE_TIMEOUT: Duration = Duration::from_millis(900);
const VOICE_MINIMUM_SPEECH: Duration = Duration::from_millis(120);

fn position_character_window(window: &CharacterWindow) {
    #[cfg(target_os = "windows")]
    {
        #[link(name = "user32")]
        unsafe extern "system" {
            fn GetSystemMetrics(index: i32) -> i32;
        }
        let width = unsafe { GetSystemMetrics(0) }.max(360);
        let height = unsafe { GetSystemMetrics(1) }.max(420);
        let x = (width - 380).max(0);
        let y = (height - 460).max(0);
        window.window().set_position(slint::PhysicalPosition::new(x, y));
    }
    #[cfg(not(target_os = "windows"))]
    {
        window.window().set_position(slint::PhysicalPosition::new(20, 20));
    }
}

fn clamp_character_position(window: &CharacterWindow, dx: f32, dy: f32, origin: &Arc<Mutex<(i32, i32)>>) {
    let (ox, oy) = origin.lock().map(|p| *p).unwrap_or((0, 0));
    let width = 360i32;
    let height = 420i32;
    #[cfg(target_os = "windows")]
    let (screen_w, screen_h) = {
        #[link(name = "user32")]
        unsafe extern "system" {
            fn GetSystemMetrics(index: i32) -> i32;
        }
        (unsafe { GetSystemMetrics(0) }.max(width), unsafe { GetSystemMetrics(1) }.max(height))
    };
    #[cfg(not(target_os = "windows"))]
    let (screen_w, screen_h) = (1920, 1080);
    let x = (ox + dx.round() as i32).clamp(0, (screen_w - width).max(0));
    let y = (oy + dy.round() as i32).clamp(0, (screen_h - height).max(0));
    window.window().set_position(slint::PhysicalPosition::new(x, y));
}

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

fn schedule_character_action(
    character: Arc<Mutex<CharacterRuntime>>,
    weak: slint::Weak<AppWindow>,
    character_weak: slint::Weak<CharacterWindow>,
    delay: Duration,
) {
    slint::Timer::single_shot(delay, move || {
        let Ok(mut runtime) = character.lock() else { return; };
        if runtime.visibility() != CharacterVisibility::Visible || runtime.is_paused() {
            return;
        }
        match runtime.render_next() {
            Ok(Some(image)) => {
                update_character_image(weak.clone(), image.clone());
                update_character_window_image(character_weak.clone(), image);
            }
            Ok(None) => {}
            Err(error) => show_error_if_alive(&weak, &format!("Character runtime error: {error}")),
        }
        drop(runtime);
        schedule_character_action(character, weak, character_weak, Duration::from_millis(1200));
    });
}

fn update_character_window_image(weak: slint::Weak<CharacterWindow>, pixels: slint::SharedPixelBuffer<slint::Rgba8Pixel>) {
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(window) = weak.upgrade() {
            window.set_character_image(slint::Image::from_rgba8(pixels));
        }
    });
}

fn update_character_image(weak: slint::Weak<AppWindow>, pixels: slint::SharedPixelBuffer<slint::Rgba8Pixel>) {
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(window) = weak.upgrade() {
            window.set_character_image(slint::Image::from_rgba8(pixels));
        }
    });
}

fn clear_character_image(window: &AppWindow) {
    let pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(2, 2);
    window.set_character_image(slint::Image::from_rgba8(pixels));
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


pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let window = AppWindow::new()?;
    let character_window = CharacterWindow::new()?;
    let storage = Storage::open_default()?;
    let settings = storage.load_settings()?;
    let settings = Arc::new(Mutex::new(settings));
    window.set_settings_ai_provider(settings.lock().unwrap().ai_provider.clone().into());
    window.set_settings_stt_provider(settings.lock().unwrap().stt_provider.clone().into());
    window.set_settings_tts_provider(settings.lock().unwrap().tts_provider.clone().into());
    window.set_settings_ai_model(settings.lock().unwrap().ai_model.clone().into());
    window.set_settings_stt_model(settings.lock().unwrap().stt_model.clone().into());
    window.set_settings_tts_model(settings.lock().unwrap().tts_model.clone().into());
    window.set_settings_tts_voice(settings.lock().unwrap().tts_voice.clone().into());
    if settings.lock().unwrap().character_paused {
        window.set_motion_button_text("Resume Motion".into());
    }
    if settings.lock().unwrap().openai_api_key.is_some() {
        window.set_settings_api_key_status("OpenAI key saved in Windows Credential Manager".into());
    }
    let persisted_messages = storage.load_messages().unwrap_or_default();
    let core = Arc::new(Mutex::new(SaeedCore::from_messages(persisted_messages.clone())));
    if let Some(last) = persisted_messages.last() {
        window.set_conversation(format!("{}: {}", last.role, last.content).into());
    }
    let character = Arc::new(Mutex::new(CharacterRuntime::new(520, 520)));
    let drag_origin = Arc::new(Mutex::new((0i32, 0i32)));
    {
        let mut runtime = character.lock().map_err(|_| "Character runtime is unavailable.")?;
        match runtime.set_visibility(CharacterVisibility::Visible) {
            Ok(()) => match runtime.render_idle() {
                Ok(Some(image)) => {
                    window.set_character_image(slint::Image::from_rgba8(image.clone()));
                    character_window.set_character_image(slint::Image::from_rgba8(image));
                    window.set_status("Ready • 3D character loaded".into());
                }
                Ok(None) => clear_character_image(&window),
                Err(error) => {
                    clear_character_image(&window);
                    window.set_character_button_text("Show Saeed".into());
                    window.set_status(format!("Ready • character unavailable: {error}").into());
                }
            },
            Err(error) => {
                clear_character_image(&window);
                window.set_character_button_text("Show Saeed".into());
                window.set_status(format!("Ready • character unavailable: {error}").into());
            }
        }
    }
    // Real character behavior is scheduled as one action at a time. No permanent timer or
    // render loop is kept alive while the character is hidden or paused.
    let _ = character_window.show();
    position_character_window(&character_window);
    schedule_character_action(
        Arc::clone(&character),
        window.as_weak(),
        character_window.as_weak(),
        Duration::from_secs(2),
    );

    {
        let origin = Arc::clone(&drag_origin);
        let weak = character_window.as_weak();
        character_window.on_drag_start(move || {
            if let Some(window) = weak.upgrade() {
                if let Ok(mut p) = origin.lock() { *p = (window.window().position().x, window.window().position().y); }
            }
        });
    }
    {
        let origin = Arc::clone(&drag_origin);
        let weak = character_window.as_weak();
        character_window.on_drag_move(move |dx, dy| {
            if let Some(window) = weak.upgrade() {
                clamp_character_position(&window, dx, dy, &origin);
            }
        });
    }
    {
        let character = Arc::clone(&character);
        let weak_main = window.as_weak();
        let weak_character = character_window.as_weak();
        character_window.on_toggle_character(move || {
            if let Ok(mut runtime) = character.lock() {
                if runtime.visibility() == CharacterVisibility::Visible {
                    let _ = runtime.set_visibility(CharacterVisibility::Hidden);
                    if let Some(w) = weak_character.upgrade() { let _ = w.hide(); }
                    if let Some(w) = weak_main.upgrade() { w.set_character_button_text("Show Saeed".into()); }
                } else if runtime.set_visibility(CharacterVisibility::Visible).is_ok() {
                    if let Some(w) = weak_character.upgrade() {
                        let _ = w.show();
                        if let Ok(Some(image)) = runtime.render_idle() { w.set_character_image(slint::Image::from_rgba8(image)); }
                    }
                    if let Some(w) = weak_main.upgrade() { w.set_character_button_text("Hide Saeed".into()); }
                }
            }
        });
    }

    let voice_active = Arc::new(AtomicBool::new(false));
    let voice_worker_running = Arc::new(AtomicBool::new(false));
    let weak = window.as_weak();

    {
        let character = Arc::clone(&character);
        let character_window_weak = character_window.as_weak();
        let weak = window.as_weak();
        window.on_toggle_character(move || {
            let Ok(mut runtime) = character.lock() else { return; };
            let next = match runtime.visibility() {
                CharacterVisibility::Visible => CharacterVisibility::Hidden,
                CharacterVisibility::Hidden => CharacterVisibility::Visible,
            };
            let result = runtime.set_visibility(next);
            let Some(window) = weak.upgrade() else { return; };
            match (next, result) {
                (CharacterVisibility::Hidden, Ok(())) => {
                    clear_character_image(&window);
                    window.set_character_button_text("Show Saeed".into());
                    window.set_status("Saeed hidden • renderer unloaded".into());
                }
                (CharacterVisibility::Visible, Ok(())) => {
                    window.set_character_button_text("Hide Saeed".into());
                    if !runtime.is_paused() {
                        schedule_character_action(Arc::clone(&character), weak.clone(), character_window_weak.clone(), Duration::from_millis(900));
                    }
                    match runtime.render_idle() {
                        Ok(Some(image)) => {
                            window.set_character_image(slint::Image::from_rgba8(image));
                            window.set_status("Saeed visible • 3D ready".into());
                        }
                        Ok(None) => clear_character_image(&window),
                        Err(error) => window.set_status(format!("Character load error: {error}").into()),
                    }
                }
                (_, Err(error)) => {
                    window.set_status(format!("Character error: {error}").into());
                }
            }
        });
    }

    {
        let character = Arc::clone(&character);
        let character_window_weak = character_window.as_weak();
        let settings = Arc::clone(&settings);
        let weak = window.as_weak();
        window.on_toggle_motion_pause(move || {
            let Ok(mut runtime) = character.lock() else { return; };
            let paused = !runtime.is_paused();
            runtime.set_paused(paused);
            let mut next_settings = settings.lock().ok().map(|s| s.clone());
            if let Some(ref mut s) = next_settings { s.character_paused = paused; }
            if let Some(s) = next_settings {
                if let Ok(storage) = Storage::open_default() { let _ = storage.save_settings(&s); }
                if let Ok(mut current) = settings.lock() { *current = s; }
            }
            if let Some(window) = weak.upgrade() {
                window.set_motion_button_text(if paused { "Resume Motion" } else { "Pause Motion" }.into());
                window.set_status(if paused { "Motion paused • manual/rest pose active" } else { "Motion resumed • procedural behavior active" }.into());
                if let Ok(Some(image)) = runtime.render_idle() { window.set_character_image(slint::Image::from_rgba8(image)); }
                if !paused {
                    schedule_character_action(Arc::clone(&character), weak.clone(), character_window_weak.clone(), Duration::from_millis(500));
                }
            }
        });
    }

    {
        let character = Arc::clone(&character);
        let weak = window.as_weak();
        window.on_set_character_state(move |state: SharedString| {
            let state = match state.as_str() {
                "Curious" => CharacterState::Curious,
                "Playful" => CharacterState::Playful,
                "Tired" => CharacterState::Tired,
                "Sleeping" => CharacterState::Sleeping,
                "Waking" => CharacterState::Waking,
                _ => CharacterState::Idle,
            };
            if let Ok(mut runtime) = character.lock() {
                runtime.set_state(state);
                if let Ok(Some(image)) = runtime.render_next() { update_character_image(weak.clone(), image); }
                if let Some(window) = weak.upgrade() { window.set_character_state_text(format!("{state:?}").into()); }
            }
        });
    }

    {
        let character = Arc::clone(&character);
        let weak = window.as_weak();
        window.on_apply_pose(move |yaw, pitch, roll, arm| {
            let parse = |v: &str| v.trim().parse::<f32>().map_err(|_| format!("Invalid pose value: {v}"));
            let values = (parse(yaw.as_str()), parse(pitch.as_str()), parse(roll.as_str()), parse(arm.as_str()));
            match values {
                (Ok(y), Ok(p), Ok(r), Ok(a)) => {
                    if let Ok(mut runtime) = character.lock() {
                        runtime.set_manual_pose(y, p, r, a);
                        runtime.set_paused(true);
                        if let Ok(Some(image)) = runtime.render_idle() { update_character_image(weak.clone(), image); }
                    }
                    if let Some(window) = weak.upgrade() { window.set_motion_button_text("Resume Motion".into()); window.set_status("Manual pose applied and motion paused.".into()); }
                }
                _ => { if let Some(window) = weak.upgrade() { window.set_status("Pose values must be valid numbers.".into()); } }
            }
        });
    }

    {
        let character = Arc::clone(&character);
        let weak = window.as_weak();
        window.on_save_rest_pose(move || {
            if let Ok(mut runtime) = character.lock() {
                runtime.save_rest_pose();
                if let Some(window) = weak.upgrade() { window.set_status("Normal rest pose saved.".into()); }
            }
        });
    }

    {
        let character = Arc::clone(&character);
        let weak = window.as_weak();
        window.on_restore_rest_pose(move || {
            if let Ok(mut runtime) = character.lock() {
                runtime.restore_rest_pose();
                runtime.set_paused(true);
                if let Ok(Some(image)) = runtime.render_idle() { update_character_image(weak.clone(), image); }
                if let Some(window) = weak.upgrade() { window.set_motion_button_text("Resume Motion".into()); window.set_status("Saved rest pose restored.".into()); }
            }
        });
    }

    {
        let core = Arc::clone(&core);
        let weak = window.as_weak();
        window.on_clear_conversation(move || {
            match Storage::open_default().and_then(|storage| storage.clear_conversation()) {
                Ok(()) => {
                    if let Ok(mut current) = core.lock() { *current = SaeedCore::new(); }
                    if let Some(window) = weak.upgrade() {
                        window.set_conversation("Saeed is ready. Shared conversation cleared.".into());
                        window.set_status("Conversation cleared.".into());
                    }
                }
                Err(error) => if let Some(window) = weak.upgrade() { window.set_status(format!("Could not clear conversation: {error}").into()); }
            }
        });
    }

    {
        let settings = Arc::clone(&settings);
        let weak = window.as_weak();
        window.on_save_settings(move |api_key, groq_key, elevenlabs_key, ai_provider, stt_provider, tts_provider, ai_model, stt_model, tts_model, tts_voice| {
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
            if !key.is_empty() { next.openai_api_key = Some(key.to_string()); }
            let groq = groq_key.trim();
            if !groq.is_empty() { next.groq_api_key = Some(groq.to_string()); }
            let eleven = elevenlabs_key.trim();
            if !eleven.is_empty() { next.elevenlabs_api_key = Some(eleven.to_string()); }
            next.ai_provider = ai_provider.trim().to_ascii_lowercase();
            next.stt_provider = stt_provider.trim().to_ascii_lowercase();
            next.tts_provider = tts_provider.trim().to_ascii_lowercase();
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
                    window.set_settings_status("Providers must be openai/groq/local (AI), openai/groq/local (STT), or openai/elevenlabs/local (TTS), and all model/voice fields are required.".into());
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
                *current = next.clone();
            }

            if let Some(window) = weak.upgrade() {
                window.set_settings_ai_provider(next.ai_provider.clone().into());
                window.set_settings_stt_provider(next.stt_provider.clone().into());
                window.set_settings_tts_provider(next.tts_provider.clone().into());
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
        let character_for_chat = Arc::clone(&character);
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
            let character = Arc::clone(&character_for_chat);
            if let Ok(mut runtime) = character.lock() {
                runtime.set_state(CharacterState::Interacting);
                if let Ok(Some(image)) = runtime.render_next() {
                    update_character_image(weak.clone(), image);
                }
            }
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
                    let messages = {
                        let mut core = core.lock()
                            .map_err(|_| "Core state is unavailable.".to_string())?;
                        core.add_chat_message("user", &text);
                        if let Some(message) = core.messages().last() {
                            if let Ok(storage) = Storage::open_default() {
                                storage.append_message(message)
                                    .map_err(|error| format!("Could not persist message: {error}"))?;
                            }
                        }
                        core.snapshot_messages()
                    };

                    let response = match settings.ai_provider.trim().to_ascii_lowercase().as_str() {
                        "local" => LocalCommandAiProvider::from_environment()?.complete(&AiRequest { messages })?,
                        "groq" => {
                            let key = settings.groq_api_key.clone().ok_or_else(|| "Groq AI selected: add the Groq API key in Settings.".to_string())?;
                            OpenAiCompatibleProvider::new(key, settings.ai_model, "https://api.groq.com/openai/v1/chat/completions").complete(&AiRequest { messages })?
                        }
                        _ => {
                            let key = settings.openai_api_key.clone().ok_or_else(|| "Add your OpenAI API key in Settings first.".to_string())?;
                            OpenAiProvider::from_config(key, settings.ai_model)?.complete(&AiRequest { messages })?
                        }
                    };

                    let response_text = response.text.clone();
                    {
                        if let Ok(mut character) = character.lock() {
                            character.set_state(CharacterState::Idle);
                        }
                    }
                    {
                        let mut core = core.lock()
                            .map_err(|_| "Core state is unavailable.".to_string())?;
                        core.add_assistant_response(
                            saeed_desktop::session::MessageSource::Chat,
                            response_text.clone(),
                        );
                        if let Some(message) = core.messages().last() {
                            if let Ok(storage) = Storage::open_default() {
                                storage.append_message(message)
                                    .map_err(|error| format!("Could not persist response: {error}"))?;
                            }
                        }
                    }

                    Ok(response_text)
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
                            Err(error) => show_error(&window, error),
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
        let character = Arc::clone(&character);
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
            let character = Arc::clone(&character);
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

                    if let Ok(mut character) = character.lock() {
                        character.set_state(CharacterState::Interacting);
                    }

                    match VoiceController::process_turn(&core, &settings, &audio) {
                        Ok(result) => {
                            let transcript = result.transcript;
                            let response = result.response;
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

                            if let Ok(mut character) = character.lock() {
                                character.set_state(CharacterState::Idle);
                            }

                            if active.load(Ordering::Acquire) {
                                update_voice_ui(&weak, "Mic OFF", "Listening…");
                            }
                        }
                        Err(error) => {
                            if let Ok(mut character) = character.lock() {
                                character.set_state(CharacterState::Idle);
                            }
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

    let tray = SaeedTray::new()?;
    {
        let weak = window.as_weak();
        tray.on_show_saeed(move || { if let Some(window) = weak.upgrade() { let _ = window.show(); } });
    }
    {
        let weak = window.as_weak();
        tray.on_hide_saeed(move || { if let Some(window) = weak.upgrade() { let _ = window.hide(); } });
    }
    tray.on_exit_saeed(move || { let _ = slint::quit_event_loop(); });
    window.show()?;
    tray.show()?;
    slint::run_event_loop()?;
    Ok(())
}

