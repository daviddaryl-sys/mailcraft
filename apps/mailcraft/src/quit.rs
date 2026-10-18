//! Leaving MailCraft: the window's close button, Quit in the macOS app menu and ⌘Q, Quit from the
//! Dock (an Apple event), and the app's own quit once a question about unsaved work is answered.
//!
//! *Whether* MailCraft may go is decided here the same way on every platform ([`decide`]): a
//! message being written, or an appointment window with unsaved changes, asks first
//! (`MailApp::close_requested`), and the close waits for the answer. *How* it goes is the
//! platform's ([`on_frame`]): on macOS through `-[NSApplication terminate:]` (`craft-quit`),
//! because eframe closing the window there can crash AppKit (its Touch Bar); elsewhere eframe
//! closes the window as it always has. Either way `eframe::App::on_exit` runs and saves.

use egui::{Context, ViewportCommand};
use mailcraft_ui_egui::MailApp;

/// Once per frame, at the end of `eframe::App::logic`: the app's decision first, then the
/// platform's quit for a close nothing held. Returns whether the platform took the close over.
pub fn on_frame(ctx: &Context, app: &mut MailApp) -> bool {
    on_frame_with(ctx, app, craft_quit::on_close_request)
}

/// [`on_frame`] with the platform's take-over passed in (tests).
fn on_frame_with(ctx: &Context, app: &mut MailApp, take_over: impl FnOnce(&Context) -> bool) -> bool {
    decide(ctx, app);
    // Last, so it sees a `CancelClose` sent above and leaves that close alone.
    take_over(ctx)
}

/// The app's say on leaving, the same on every platform. A close the window was asked for (its
/// close button, the app menu's Quit, the Dock) is held with `CancelClose` while something
/// unsaved is being asked about, and the window is brought back so the question can be answered.
/// The app's own quit (`quit_requested`: the question answered, `app.quit`) becomes a close
/// request, decided on the next frame like any other.
fn decide(ctx: &Context, app: &mut MailApp) {
    if ctx.input(|i| i.viewport().close_requested()) && !app.close_requested() {
        ctx.send_viewport_cmd(ViewportCommand::CancelClose);
        if ctx.input(|i| i.viewport().minimized == Some(true)) {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
        }
        ctx.send_viewport_cmd(ViewportCommand::Focus);
    }
    if app.quit_requested {
        // Asked once: a quit that is held again asks again, and sets this again when answered.
        app.quit_requested = false;
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use egui::{RawInput, ViewportEvent, ViewportId};
    use mailcraft_engine::Session;
    use mailcraft_model::Mailbox;
    use mailcraft_ui_egui::Services;
    use serde_json::json;

    use super::*;

    fn app() -> MailApp {
        MailApp::new(Session::new(Mailbox::default()), Services::default())
    }

    /// A message being written that something was typed into.
    fn writing(app: &mut MailApp) {
        app.session.run("mail.new", &json!({})).unwrap();
        app.session.compose.as_mut().unwrap().dirty = true;
    }

    /// One logic frame of the main window (as eframe runs it, also while minimized), with its
    /// close asked for or not. Returns whether the platform's quit ran (a stand-in for craft-quit
    /// that quits whenever it is reached) and the window's commands.
    fn frame(app: &mut MailApp, close: bool, minimized: bool) -> (bool, Vec<ViewportCommand>) {
        let ctx = Context::default();
        let mut input = RawInput::default();
        let root = input.viewports.entry(ViewportId::ROOT).or_default();
        root.minimized = Some(minimized);
        if close {
            root.events.push(ViewportEvent::Close);
        }
        let quit = Cell::new(false);
        let mut out = ctx.run_logic(&input, |ctx| {
            on_frame_with(ctx, app, |ctx| {
                // craft-quit's rule: a close the app cancelled this frame is not taken over.
                let held = ctx.viewport(|v| v.commands.contains(&ViewportCommand::CancelClose));
                let go = ctx.input(|i| i.viewport().close_requested()) && !held;
                quit.set(go);
                go
            });
        });
        (quit.get(), out.viewport_commands.remove(&ViewportId::ROOT).unwrap_or_default())
    }

    #[test]
    fn nothing_unsaved_quits_at_once() {
        let mut app = app();
        let (quit, commands) = frame(&mut app, true, false);
        assert!(quit);
        assert!(!commands.contains(&ViewportCommand::CancelClose));
    }

    #[test]
    fn no_close_asked_for_does_nothing() {
        let mut app = app();
        let (quit, commands) = frame(&mut app, false, false);
        assert!(!quit && commands.is_empty());
    }

    #[test]
    fn a_message_being_written_asks_first_and_holds_the_close() {
        let mut app = app();
        writing(&mut app);
        let (quit, commands) = frame(&mut app, true, false);
        assert!(!quit, "the question comes before the quit");
        assert!(commands.contains(&ViewportCommand::CancelClose));
        assert!(app.dialog.is_some(), "the discard question is open");
        assert!(!app.quit_requested);
    }

    #[test]
    fn a_held_close_brings_a_minimized_window_back() {
        let mut app = app();
        writing(&mut app);
        let (_, commands) = frame(&mut app, true, true);
        assert!(commands.contains(&ViewportCommand::Minimized(false)));
        assert!(commands.contains(&ViewportCommand::Focus));
    }

    #[test]
    fn the_question_answered_closes_and_then_quits() {
        let mut app = app();
        writing(&mut app);
        frame(&mut app, true, false);
        // Don't Save: the draft goes, and the app asks to quit.
        app.session.compose = None;
        app.dialog = None;
        app.quit_requested = true;
        let (quit, commands) = frame(&mut app, false, false);
        assert!(!quit);
        assert!(commands.contains(&ViewportCommand::Close), "the close is asked for again");
        assert!(!app.quit_requested, "asked once");
        // The next frame decides that close like any other: nothing is left to ask about.
        let (quit, commands) = frame(&mut app, true, false);
        assert!(quit);
        assert!(!commands.contains(&ViewportCommand::CancelClose));
    }

    #[test]
    fn this_platform_quits_the_way_craft_quit_says() {
        // macOS takes the close over (`terminate:`); elsewhere eframe closes the window.
        let ctx = Context::default();
        let mut input = RawInput::default();
        input.viewports.entry(ViewportId::ROOT).or_default().events.push(ViewportEvent::Close);
        let mut app = app();
        writing(&mut app);
        let mut taken = true;
        let _ = ctx.run_logic(&input, |ctx| taken = on_frame(ctx, &mut app));
        assert!(!taken, "a held close is never taken over, on any platform");
        #[cfg(not(target_os = "macos"))]
        {
            let mut app = self::app();
            let mut taken = true;
            let _ = ctx.run_logic(&input, |ctx| taken = on_frame(ctx, &mut app));
            assert!(!taken, "no platform quit here: the window closes as it always has");
        }
    }
}
