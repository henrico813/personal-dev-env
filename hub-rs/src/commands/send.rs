//! Runs `hub send` and `hub inbox`. The message rules themselves live
//! in `shared/messages.rs`, which `hub next` shares.

use jiff::Zoned;

use super::Outcome;
use crate::external::cards::CardKey;
use crate::external::herdr::Herdr;
use crate::shared::events::{self, Event, EventKind, Via};
use crate::shared::messages::{self, Delivery, SendOutcome, SendState, StoredMessage, Target};
use crate::shared::run_folder::RunFolder;
use crate::shared::{AgentName, HubError, Tools};

/// One message to send, with the text already read from MESSAGE or
/// `--file`.
pub struct SendRequest {
    pub name: AgentName,
    /// The saved Herdr machine from `--machine`, or `None` for this
    /// one. `main` already points `tools.herdr` at the chosen machine.
    /// A held message keeps that machine's label, so `hub next` sends
    /// it there too.
    pub machine: Option<String>,
    pub text: String,
}

/// Sends the message now when the session can take it. Otherwise it
/// holds the message behind any older one, or sends it to the session
/// that took over the card after a handoff. The message file is
/// written before Herdr is called, so a crash leaves it as `.sending`
/// and the text stays on disk.
///
/// # Errors
///
/// `HubError::NotSent` when Herdr turned it down before taking it, or
/// when the session is gone. `HubError::Unsure` when it may have
/// arrived. `HubError::PartlyDone` when the message was held or sent
/// and a later step failed.
pub fn run_send(
    tools: &Tools<'_>,
    run: &RunFolder,
    request: &SendRequest,
    now: &Zoned,
) -> Result<Outcome, HubError> {
    let name = request.name.as_str();
    let target = match tools.herdr.get_agent(name) {
        Ok(agent) => Target::Live(agent),
        // Only a name with a card key can have a successor on its card.
        Err(error) if error.is_agent_not_found() => match CardKey::find_in_name(name) {
            Some(_) => {
                messages::find_card_successor(tools.cards, name, &tools.herdr.list_agents()?)
            }
            None => Target::Gone,
        },
        Err(error) => return Err(error.into()),
    };
    let older_waiting = messages::list_messages(run, Some(name))?.len();
    let machine = request.machine.as_deref();
    match messages::choose_delivery(&target, older_waiting) {
        Delivery::Now => {
            let stored = messages::write_message(
                run,
                name,
                machine,
                &request.text,
                SendState::Sending,
                now,
            )?;
            deliver_now(tools.herdr, run, stored, now)?;
            Ok(Outcome::line(format!("sent {name}")))
        }
        Delivery::Hold(why) => {
            let stored =
                messages::write_message(run, name, machine, &request.text, SendState::Held, now)?;
            let held = Event {
                at: now.timestamp(),
                kind: EventKind::Held {
                    name: name.to_owned(),
                    id: stored.id,
                },
            };
            events::append_event(run, &held).map_err(|error| HubError::PartlyDone {
                done: format!("held {name}"),
                error: Box::new(error),
            })?;
            Ok(Outcome::line(format!("held {name}: {}", why.words())))
        }
        Delivery::MoveTo(successor) => {
            let stored = messages::write_message(
                run,
                &successor,
                machine,
                &request.text,
                SendState::Sending,
                now,
            )?;
            let moved = Event {
                at: now.timestamp(),
                kind: EventKind::Moved {
                    name: name.to_owned(),
                    id: stored.id.clone(),
                    to: successor.clone(),
                },
            };
            events::append_event(run, &moved)?;
            deliver_now(tools.herdr, run, stored, now)?;
            Ok(Outcome::line(format!(
                "sent {successor} ({name} handed off to it)"
            )))
        }
        Delivery::Gone => Err(HubError::NotSent {
            name: name.to_owned(),
            reason: "the session is gone".to_owned(),
        }),
    }
}

/// Prints every message held for the session and counts each as
/// delivered, so `hub next` never sends it again. A worker runs it
/// before it writes its handoff.
pub fn run_inbox(run: &RunFolder, name: &AgentName, now: &Zoned) -> Result<Outcome, HubError> {
    let name = name.as_str();
    let taken = messages::take_held_messages(run, name, now)?;
    let mut lines = vec![match taken.len() {
        0 => format!("no held messages for {name}"),
        count => format!("{count} held messages for {name}"),
    }];
    for message in taken {
        let created = message.created.to_zoned(now.time_zone().clone());
        lines.push(format!("--- {}", created.strftime("%H:%M")));
        lines.extend(message.text.lines().map(str::to_owned));
    }
    Ok(Outcome { lines })
}

/// Delivers a message that `hub send` wrote. Returns an error when the
/// message did not arrive, or may have arrived without confirming it.
fn deliver_now(
    herdr: &dyn Herdr,
    run: &RunFolder,
    stored: StoredMessage,
    now: &Zoned,
) -> Result<(), HubError> {
    match messages::deliver_message(herdr, run, stored, Via::Send, now)? {
        SendOutcome::Delivered => Ok(()),
        SendOutcome::NotSent { name, reason } => Err(HubError::NotSent { name, reason }),
        SendOutcome::Unsure { name, reason, file } => Err(HubError::Unsure { name, reason, file }),
    }
}
