use crate::{
    error::{CompileError, RuntimeError},
    lang::{compile, Machine, MachineState},
    models::Environment,
};
use actix::{Actor, ActorContext, AsyncContext, StreamHandler};
use actix_web::{web, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use serde::{Deserialize, Serialize};
use std::{
    convert,
    time::{Duration, Instant},
};

/// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

/// All the different types of events that we can receive over the websocket.
/// These events are typically triggered by user input, but might not
/// necessarily be.
#[derive(Debug, Deserialize)]
#[serde(
    tag = "type",
    content = "content",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum IncomingEvent {
    Edit {
        // Saving room for more fields here
        source: String,
    },
    Compile,
    Step,
}

/// All the different types of events that we can transmit over the websocket.
/// This can include both success and error events.
#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "content", rename_all = "snake_case")]
enum OutgoingEvent<'a> {
    // OK events
    /// Send latest version of the program source code
    Source {
        // Saving room for more fields here
        source: &'a str,
    },
    /// Send latest version of the machine state
    MachineState {
        state: &'a MachineState,
        is_complete: bool,
        is_successful: bool,
    },

    // Error events
    /// Failed to parse websocket message
    MalformedMessage(String),
    /// Failed to parse the sent program
    CompileError(CompileError),
    /// Error occurred while running a program
    RuntimeError(RuntimeError),
    /// "Step" message occurred before "Compile" message
    NoCompilation,
}

// Define type conversions to make processing code a bit cleaner

impl<'a> From<&'a Machine> for OutgoingEvent<'a> {
    fn from(other: &'a Machine) -> Self {
        OutgoingEvent::MachineState {
            state: other.get_state(),
            is_complete: other.is_complete(),
            is_successful: other.is_successful(),
        }
    }
}

impl<'a> From<serde_json::Error> for OutgoingEvent<'a> {
    fn from(other: serde_json::Error) -> Self {
        OutgoingEvent::MalformedMessage(format!("{}", other))
    }
}

impl<'a> From<CompileError> for OutgoingEvent<'a> {
    fn from(other: CompileError) -> Self {
        OutgoingEvent::CompileError(other)
    }
}

impl<'a> From<RuntimeError> for OutgoingEvent<'a> {
    fn from(other: RuntimeError) -> Self {
        OutgoingEvent::RuntimeError(other)
    }
}

/// The controlling struct for a single websocket instance
struct ProgramWebsocket {
    /// Track the last time we pinged/ponged the client, if this exceeds
    /// CLIENT_TIMEOUT, drop the connection
    heartbeat: Instant,
    /// The current user-entered program source code
    source_code: String,
    /// The current execution state of the machine. None if the program hasn't
    /// been compiled yet.
    machine: Option<Machine>,
}

impl ProgramWebsocket {
    fn new() -> Self {
        ProgramWebsocket {
            heartbeat: Instant::now(),
            source_code: String::new(),
            machine: None,
        }
    }

    /// Processes the given text message, and returns the appropriate response
    /// event. The return type on this is a little funky because all our
    /// event types (OK and error) are under the same enum. We still use a
    /// Result because it makes it easier to exit early in the case of an error.
    fn process_msg(
        &mut self,
        text: String,
    ) -> Result<OutgoingEvent, OutgoingEvent> {
        // Parse the message
        let socket_msg = serde_json::from_str::<IncomingEvent>(&text)?;

        // Process message based on type
        Ok(match socket_msg {
            IncomingEvent::Edit { source } => {
                // Update source code
                self.source_code = source;
                // source code has changed, machine is no longer valid
                self.machine = None;
                OutgoingEvent::Source {
                    source: &self.source_code,
                }
            }
            IncomingEvent::Compile => {
                // Compile the program into a machine
                let env = Environment {
                    num_stacks: 0,
                    max_stack_size: None,
                    input: vec![1, 2, 3],
                    expected_output: vec![1, 2, 3],
                }; // TODO read from DB

                // Clone the source so the parsing doesn't mutate our copy
                let src_copy = self.source_code.clone();
                self.machine = Some(compile(env, &mut src_copy.as_bytes())?);

                // we need this fuckery cause lol borrow checker
                self.machine.as_ref().unwrap().into()
            }
            IncomingEvent::Step => {
                // Execute one step on the machine
                if let Some(machine) = self.machine.as_mut() {
                    machine.execute_next()?;
                    (&*machine).into() // need to convert &mut to just &
                } else {
                    return Err(OutgoingEvent::NoCompilation);
                }
            }
        })
    }
}

impl Actor for ProgramWebsocket {
    type Context = ws::WebsocketContext<Self>;

    /// Method is called on actor start. Kick off an interval that pings the
    /// client periodically and also checks if they have timed out.
    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            // Check if client has timed out
            if Instant::now().duration_since(act.heartbeat) > CLIENT_TIMEOUT {
                // Timed out, kill the connection
                ctx.stop();
            } else {
                // Not timed out, send another ping
                ctx.ping("");
            }
        });
    }
}

/// Handler for `ws::Message`
impl StreamHandler<ws::Message, ws::ProtocolError> for ProgramWebsocket {
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
        // process websocket messages
        match msg {
            ws::Message::Ping(msg) => {
                self.heartbeat = Instant::now();
                ctx.pong(&msg);
            }
            ws::Message::Pong(_) => {
                self.heartbeat = Instant::now();
            }
            ws::Message::Text(text) => {
                // This is a little funky because both sides of the Result are
                // the same type
                let response =
                    self.process_msg(text).unwrap_or_else(convert::identity);
                let response_string = serde_json::to_string(&response).unwrap();

                ctx.text(response_string);
            }
            ws::Message::Binary(_) => {}
            ws::Message::Close(_) => {
                ctx.stop();
            }
            ws::Message::Nop => {}
        }
    }
}

/// do websocket handshake and start `ProgramWebsocket` actor
pub fn ws_index(
    r: HttpRequest,
    stream: web::Payload,
) -> Result<HttpResponse, actix_web::Error> {
    ws::start(ProgramWebsocket::new(), &r, stream)
}