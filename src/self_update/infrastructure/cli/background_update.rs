//! The background update check that starts with every terminal tab (`_ pty`, decision 4).

use crate::self_update::application::update_trigger::UpdateTrigger;
use crate::self_update::domain::value_objects::update_outcome::UpdateOutcome;
use crate::self_update::infrastructure::self_update_module::SelfUpdateModule;
use crate::shared::infrastructure::logger;

const THREAD_NAME: &str = "self-update";

/// Starts the update check on its own thread and returns at once: the session never waits for it. The thread builds
/// its own module with `build`, since the modules hold `Rc`, which cannot cross threads. What happens goes to
/// `pty.log`, never to the terminal; a new version takes effect in the next tab.
pub(crate) fn start_background_update(build: fn() -> Option<SelfUpdateModule>) {
    log_panics_of_the_update_thread();
    let spawned = std::thread::Builder::new().name(THREAD_NAME.to_string()).spawn(move || {
        if std::panic::catch_unwind(|| check_for_update(build)).is_err() {
            logger::error("self-update: the background check stopped on a panic");
        }
    });
    if let Err(error) = spawned {
        logger::warn(&format!("self-update: could not start the background check: {error}"));
    }
}

fn check_for_update(build: fn() -> Option<SelfUpdateModule>) {
    let Some(module) = build() else {
        return;
    };
    match module.update_tabsmith.execute(UpdateTrigger::Scheduled) {
        // At warn, the level pty.log keeps by default, so the update can be found there.
        Ok(UpdateOutcome::Updated { from, to }) => {
            logger::warn(&format!("self-update: updated tabsmith from {from} to {to}; new tabs run it"))
        }
        Ok(outcome) => logger::debug(&format!("self-update: {outcome:?}")),
        Err(error) => logger::warn(&format!("self-update: {error}")),
    }
}

/// The default panic hook prints to stderr, which is the user's terminal until the session installs its own
/// logging hook a moment later. Until then a panic on the update thread is logged instead; other threads keep the
/// hook they had.
fn log_panics_of_the_update_thread() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if std::thread::current().name() == Some(THREAD_NAME) {
            logger::error(&format!("self-update: panic: {info}"));
        } else {
            previous(info);
        }
    }));
}
