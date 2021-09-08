use crate::prelude::*;
pub use crate::{Error, Result};

const MSGBOX: usize = 1024;

//#[derive(Message)]
//#[rtype(result = "()")]
//struct ActiveTick(());

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub enum IncomingMsg {
    Msg { msg: Arc<Vec<u8>> },
}

struct SocketActor {
    //r: ReadHalf<TcpStream>,
    receive_tx: broadcast::Sender<IncomingMsg>,
}

impl Actor for SocketActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Context<Self>) {
        ()
    }

    fn stopped(&mut self, _ctx: &mut Context<Self>) {
        ()
    }
}

impl Handler<IncomingMsg> for SocketActor {
    type Result = ();

    fn handle(&mut self, msg: IncomingMsg, _ctx: &mut Context<Self>) -> Self::Result {
        self.receive_tx.send(msg).unwrap(); //FIXME: error handling
        ()
    }
}

pub struct Socket {
    receive_tx: broadcast::Sender<IncomingMsg>,
    w: WriteHalf<TcpStream>,
}

impl Socket {
    pub async fn connect(addr: SocketAddr) -> Result<Socket> {
        let stream = TcpStream::connect(addr).await?;
        start_stream(stream)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<IncomingMsg> {
        return self.receive_tx.subscribe();
    }

    pub async fn send(&mut self, msg: &[u8]) -> Result<()> {
        self.w.write(&msg).await?;
        Ok(())
    }
}

pub struct Listener {
    listener: TcpListener,
}

impl Listener {
    pub async fn bind(addr: SocketAddr) -> Result<Listener> {
        let listener = TcpListener::bind(addr).await?;
        Ok(Listener { listener })
    }

    pub async fn accept(&self) -> Result<Socket> {
        let (stream, _) = self.listener.accept().await?;
        start_stream(stream)
    }
}

fn start_stream(stream: TcpStream) -> Result<Socket> {
    let (mut r, w) = tokio::io::split(stream);

    let (receive_tx, _receive_rx) = broadcast::channel(MSGBOX);
    let socket_receive_tx = receive_tx.clone();

    let execution = async move {
        // `Actor::start` spawns the `Actor` on the *current* `Arbiter`, which
        // in this case is the System arbiter
        let addr = SocketActor { receive_tx }.start();

        let mut buf = BytesMut::with_capacity(1024);
        match r.read_buf(&mut buf).await {
            Ok(0) => (),
            Ok(_) => {
                let payload = buf.to_vec();
                addr.send(IncomingMsg::Msg {
                    msg: Arc::new(payload),
                })
                .await
                .unwrap(); // FIXME: handle error?
            }
            Err(_) => return (),
        }
    };
    let arb = Arbiter::new();
    // Spawn the future onto the current Arbiter/event loop
    arb.spawn(execution);

    Ok(Socket {
        receive_tx: socket_receive_tx,
        w,
    })
}
