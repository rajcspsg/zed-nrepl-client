use gpui::*;

use crate::nrepl_client::NreplClient;
use crate::repl::{NreplRequest, NreplRequestResponse};

#[derive(Clone)]
pub struct State {
    pub count: usize, // hack for generating req-resp item ids
    pub items: Vec<NreplRequestResponse>,
}

#[derive(Clone)]
pub struct StateModel {
    pub inner: Entity<State>,
    pub client: Entity<NreplClient>,
}

impl StateModel {
    pub fn init(app: &mut App, port: u16) {
        let model = app.new(|_cx| State {
            count: 0,
            items: vec![],
        });

        let mut client = app.new(|_cx| match NreplClient::connect("127.0.0.1", port) {
            Ok(c) => c,
            Err(e) => {
                println!("Failed to connect: {}", e);
                panic!(
                    "Make sure nREPL server is running with: lein repl :headless :host 127.0.0.1 :port {}",
                    port
                );
            }
        });

        let this = Self {
            inner: model,
            client: client,
        };
        app.set_global(this.clone());
    }

    pub fn update(f: impl FnOnce(&mut Self, &mut App), cx: &mut App) {
        if !cx.has_global::<Self>() {
            return;
        }
        cx.update_global::<Self, _>(|mut this, cx| {
            f(&mut this, cx);
        });
    }

    pub fn push(&self, item: NreplRequest, cx: &mut App) {
        self.inner.update(cx, |model, cx| {
            // let nrepl = Nrepl::global(cx);
            // let client = &nrepl.client;
            //let eval_result = client.eval("(+ 1 2 3)");
            self.client.update(cx, |client, _| {
                let result = client.eval(item.req.trim());
                match result {
                    Ok(v) => {
                        if let Some(value) = &v.value {
                            println!("  Value: {}", value);
                            let repl_entry = NreplRequestResponse {
                                id: item.id,
                                req: item.req,
                                resp: value.into(),
                            };
                            model.items.push(repl_entry);
                        }
                        if !v.output.is_empty() {
                            println!("  Output: '{}'", v.output);
                        }
                    }
                    Err(e) => println!("error occured {}", e),
                }
            });
            //println!();
            model.count += 1;
            cx.emit(ListChangedEvent {});
        });
    }

    pub fn remove(&self, id: usize, cx: &mut App) {
        self.inner.update(cx, |model, cx| {
            let index = model.items.iter().position(|x| x.id == id).unwrap();
            model.items.remove(index);
            cx.emit(ListChangedEvent {});
        });
    }
}

impl Global for StateModel {}

#[derive(Clone, Debug)]
pub struct ListChangedEvent {}

impl EventEmitter<ListChangedEvent> for State {}

pub struct Nrepl {
    client: NreplClient,
}

impl Global for Nrepl {}

impl Clone for Nrepl {
    fn clone(&self) -> Self {
        let port = self.client.get_port();
        let mut client = match NreplClient::connect("127.0.0.1", port) {
            Ok(c) => c,
            Err(e) => {
                println!("Failed to connect: {}", e);
                panic!(
                    "Make sure nREPL server is running with: lein repl :headless :host 127.0.0.1 :port {}",
                    port
                );
            }
        };
        Nrepl { client: client }
    }
}
impl Nrepl {
    pub fn init(app: &mut App) {
        let mut client = match NreplClient::connect("127.0.0.1", 64649) {
            Ok(c) => c,
            Err(e) => {
                println!("Failed to connect: {}", e);
                panic!(
                    "Make sure nREPL server is running with: lein repl :headless :host 127.0.0.1 :port 63067"
                );
            }
        };

        let nrepl = Nrepl { client: client };

        app.set_global(nrepl);
    }
}
