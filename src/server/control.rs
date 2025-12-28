use std::pin::{Pin};
use std::task::{Context, Poll};
use actix_web::{web, Handler, HttpRequest, HttpResponse, Scope};
use actix_ws::{Message, MessageStream, Session};
use futures_util::{SinkExt, StreamExt as _};
use futures_util::task::SpawnExt;
use flume::Receiver;
use futures_util::future::Either;
use log::info;

pub fn build_control_scope<T: Clone + 'static>(rx: Receiver<T>) -> Scope {
    Scope::new("")
        .service(web::resource("/ws").route(web::get().to(WSHandler{rx})))
}

#[derive(Debug, Clone)]
struct WSHandler<T: Clone + 'static> {
    rx: Receiver<T>,
}

impl<T: Clone + 'static> WSHandler<T> {
    fn spawn(&self, mut msg_stream: MessageStream, mut session: Session) {
        let rx = self.rx.clone();
        actix_web::rt::spawn(async move {
            loop {
                match futures::future::select(msg_stream.next(), rx.recv_async()).await {
                    Either::Left((Some(Ok(msg)), _)) => {
                        match msg {
                            Message::Ping(bytes) => {
                                if session.pong(&bytes).await.is_err() {
                                    return;
                                }
                            }

                            Message::Text(msg) => info!("Got websocket message, ignoring: {msg}"),
                            _ => break,
                        }
                    }
                    Either::Left((Some(Err(_)), _)) => break,
                    Either::Left((None, _)) => break,
                    Either::Right((Ok(notif), _)) => {
                        if session.text(r#"{"kind": "reload"}"#).await.is_err() {
                            break;
                        };
                    }
                    Either::Right((Err(_), _)) => break,
                }
            }

            let _ = session.close(None).await;
        });
    }
}

impl<T: Clone + 'static> Handler<(HttpRequest, web::Payload)> for WSHandler<T> {
    type Output = actix_web::Result<HttpResponse>;
    type Future = WSFuture;

    fn call(&self, (req, body): (HttpRequest, web::Payload)) -> Self::Future {
        let (response, session, msg_stream) = match actix_ws::handle(&req, body){
            Ok(v) => v,
            Err(e) =>
                return WSFuture {
                    response: Some(Err(e)),
                }
        };

        self.spawn(msg_stream, session);

        WSFuture {
            response: Some(Ok(response)),
        }
    }
}

struct WSFuture {
    response: Option<actix_web::Result<HttpResponse>>,
}
impl Future for WSFuture {
    type Output = actix_web::Result<HttpResponse>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(Pin::into_inner(self).response.take().unwrap())
    }
}