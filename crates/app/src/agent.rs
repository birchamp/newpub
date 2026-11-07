//! The desktop app as an MCP host (AG-02): `newpub --agent` serves the protocol on stdin/stdout while the window
//! shows the publication, so a person watches an agent work and can take over at any time. The tools and their
//! dispatch live in `newpub-agent`; this file keeps the view in step and protects the person's unsaved work.

use std::sync::{Arc, Mutex};

use anyhow::{Result, bail};
use newpub_agent::{Host, Link, Waker};

use crate::{Dialog, NewpubApp};

/// A live connection: the reader thread queues requests and wakes the UI, which answers them each frame.
pub struct Live {
    link: Link,
    wake: Waker,
}

impl Host for NewpubApp {
    fn session(&mut self) -> &mut newpub_engine::Session {
        &mut self.session
    }

    /// The person's unsaved work is never replaced behind their back (UI-16): the agent must save first, or ask.
    fn before_replace(&mut self) -> Result<()> {
        if self.session.dirty {
            bail!(
                "the publication has unsaved changes the person has not saved; call newpub_save first, or ask them \
                 to save or discard their work"
            );
        }
        Ok(())
    }

    /// A new or opened publication: back to page 1 with nothing selected, as the app's own Open does.
    fn after_replace(&mut self) {
        self.page = 0;
        self.selection.clear();
        self.end_text_edit();
        self.view.reset_transient();
        self.reset_gesture();
        self.thumbs.clear();
        if matches!(self.dialog, Dialog::Picker(_)) {
            self.dialog = Dialog::None;
        }
        if self.session.path.is_some() {
            self.remember_recent();
        }
    }

    /// After an agent changed the document: leave text editing (the caret may point at text that is gone), keep
    /// the page and selection valid, and tell the person what happened in the status bar.
    fn after_actions(&mut self, summary: &str) {
        self.end_text_edit();
        self.sync_after_change();
        self.status = summary.to_string();
    }
}

impl NewpubApp {
    /// Starts serving MCP on stdin/stdout (the `--agent` flag).
    pub fn attach_agent(&mut self) {
        let wake: Waker = Arc::new(Mutex::new(None));
        let link = newpub_agent::stdio_link(wake.clone());
        self.agent = Some(Live { link, wake });
    }

    /// Answers the requests an agent queued since the last frame. When the client closes the connection the
    /// window stays open with the publication, and the status bar says so.
    pub(crate) fn agent_pump(&mut self, ctx: &egui::Context) {
        let Some(live) = self.agent.take() else { return };
        if let Ok(mut w) = live.wake.lock()
            && w.is_none()
        {
            let c = ctx.clone();
            *w = Some(Box::new(move || c.request_repaint()));
        }
        let pumped = live.link.pump(self);
        if pumped.handled > 0 || pumped.closed {
            ctx.request_repaint();
        }
        if pumped.closed {
            self.status = "Agent disconnected".into();
        } else {
            self.agent = Some(live);
        }
    }
}
