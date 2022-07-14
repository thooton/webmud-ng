use std::time::{Duration, Instant};

use actix::prelude::*;
use actix_web_actors::ws;
use tokio::sync::mpsc::UnboundedSender;
use crate::{conn::{ConnParent, self}, debug};

/// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

/// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

/// websocket connection is long running connection, it easier
/// to handle with an actor
pub struct SocketHandler {
    /// Client must send ping at least once per 10 seconds (CLIENT_TIMEOUT),
    /// otherwise we drop connection.
    hb: Instant,
    tx: Option<UnboundedSender<String>>
}

#[derive(Clone)]
struct HandlerParent {
    addr: Addr<SocketHandler>
}

impl ConnParent for HandlerParent {
    fn try_send(&self, msg: String) -> () {
        self.addr.do_send(SocketSend(msg))
    }
    fn try_close_conn(&self) -> () {
        self.addr.do_send(SocketClose)
    }
}

impl SocketHandler {
    pub fn new() -> Self {
        Self { hb: Instant::now(), tx: None }
    }

    /// helper method that sends ping to client every second.
    ///
    /// also this method checks heartbeats from client
    fn hb(&self, ctx: &mut <Self as Actor>::Context) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            // check client heartbeats
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                // heartbeat timed out
                debug!("Websocket Client heartbeat failed, disconnecting!");

                // stop actor
                ctx.stop();

                // don't try to send a ping
                return;
            }

            ctx.ping(b"");
        });
    }
}

impl Actor for SocketHandler {
    type Context = ws::WebsocketContext<Self>;

    /// Method is called on actor start. We start the heartbeat process here.
    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);
        self.tx = Some(conn::start(HandlerParent { addr: ctx.address() }));
        //self.tx.init(HandlerParent { addr: ctx.address() });
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct SocketSend(pub String);

#[derive(Message)]
#[rtype(result = "()")]
pub struct SocketClose;

/// Handler for `ws::Message`
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for SocketHandler {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        // process websocket messages
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.hb = Instant::now();
            }
            Ok(ws::Message::Text(text)) => {
                match self.tx.as_ref().unwrap().send(text.into()) {
                    Err(_) => {
                        ctx.close(None);
                        ctx.stop();
                    },
                    Ok(_) => {}
                }
            }
            Ok(ws::Message::Binary(_)) => {},
            Ok(ws::Message::Close(reason)) => {
                debug!("Connection closed with {:?}", reason);
                ctx.close(reason);
                ctx.stop();
            }
            other => {
                debug!("Unknown message: {:?}", other);
                debug!("Stopping connection");
                ctx.stop()
            },
        }
    }
}

impl Handler<SocketSend> for SocketHandler {
    type Result = ();

    fn handle(&mut self, msg: SocketSend, ctx: &mut Self::Context) -> Self::Result {
        ctx.text(msg.0);
    }
}

impl Handler<SocketClose> for SocketHandler {
    type Result = ();

    fn handle(&mut self, _msg: SocketClose, ctx: &mut Self::Context) -> Self::Result {
        ctx.close(None);
        ctx.stop();
    }
}