use core::{num::NonZero, sync::atomic::{AtomicUsize, Ordering}};
use alloc::sync::Arc;

use futures_util::SinkExt;
use tokio_stream::StreamExt;
use tokio_serde::{formats::SymmetricalBincode, Framed, SymmetricallyFramed};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use tokio::{sync::Mutex, net::{unix::{OwnedReadHalf, OwnedWriteHalf}, UnixListener, UnixStream}};

macro_rules! def_serde_compatibly_omissible_config_default {
    ($ident: ident, <$ty: ty> { $($def: tt)* }) => {
        pub mod $ident {
            pub static DEFAULT: std::sync::LazyLock<$ty> = std::sync::LazyLock::new(|| { $($def)* });
            pub fn is_default(value: &$ty) -> bool { *value == *DEFAULT }
            pub fn get_default() -> &'static $ty { &*DEFAULT }
            pub fn clone_default() -> $ty { DEFAULT.clone() }
        }
    };
}

def_serde_compatibly_omissible_config_default!(socket_path, <std::path::PathBuf> {
    crate::util::APPLICATION_SUPPORT_FOLDER.join("ipc.sock")
});


pub trait PacketIdCounterSource {}
/// Marker types for packet sources.
mod s {
    #[derive(Debug)]
    pub struct Remote; impl super::PacketIdCounterSource for Remote {}
    #[derive(Debug)]
    pub struct Local; impl super::PacketIdCounterSource for Local {}
}


static PACKET_ID_COUNTER: AtomicUsize = AtomicUsize::new(1);
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[allow(clippy::unsafe_derive_deserialize, reason = "deserializer for NonZero ensures non-zero")]
pub struct PacketID<T: PacketIdCounterSource>(NonZero<usize>, core::marker::PhantomData<T>);
impl<T: PacketIdCounterSource> PacketID<T> {
    #[allow(clippy::inline_always)]
    #[inline(always)]
    pub const unsafe fn from_usize(usize: usize) -> Self {
        Self({
            #[cfg(debug_assertions)]
            { NonZero::new(usize).expect("cannot create a packet ID with a value of zero") }
            #[cfg(not(debug_assertions))]
            unsafe { NonZero::new_unchecked(usize) }
        }, core::marker::PhantomData)
    }
}
impl PacketID<s::Local> {
    pub fn new() -> Self {
        let id = PACKET_ID_COUNTER.fetch_add(1, Ordering::AcqRel);
        unsafe { Self::from_usize(id) }
    }
}

const IPC_PROTOCOL_VERSION: usize = 0;
pub mod packets {
    use super::{IPC_PROTOCOL_VERSION, s};
    use serde::{Serialize, Deserialize};

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Hello {
        /// IPC protocol version.
        pub version: usize,
        /// The process ID of the process sending this packet (the one constructing this).
        pub process: libc::pid_t,
    }
    impl Hello {
        pub fn new() -> Self {
            Self {
                version: IPC_PROTOCOL_VERSION,
                process: *crate::util::OWN_PID
            }
        }
    }
    impl From<Hello> for super::Packet {
        fn from(val: Hello) -> Self {
            Self::Hello(val)
        }
    }

    #[allow(clippy::unsafe_derive_deserialize, reason = "only casting source marker type; no runtime invariant can be broken")]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct GeneralFailure {
        /// The packet this failure is a reaction to.
        // Typed as local since that's the only time we'd care about what the ID is.
        pub reaction: Option<super::PacketID<s::Local>>,
        pub reason: String,
    }
    impl GeneralFailure {
        // ID is remote here since we'd only construct to send
        pub fn new(reaction: Option<super::PacketID<s::Remote>>, reason: impl Into<String>) -> Self {
            Self {
                reaction: unsafe {
                    core::mem::transmute::<
                        Option<super::PacketID<s::Remote>>,
                        Option<super::PacketID<s::Local>>
                    >(reaction)
                },
                reason: reason.into()
            }
        }
    }

}

#[expect(clippy::unsafe_derive_deserialize, reason = "safe transmutation of enum discriminants")]
#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[repr(u16)]
pub enum Packet {
    Hello(packets::Hello) = 0,
    GeneralFailure(packets::GeneralFailure) = 1,
    ReloadConfiguration = 2,
}
impl Packet {
    pub fn hello() -> Self {
        packets::Hello::new().into()
    }

    /// ## Safety
    /// See: <https://doc.rust-lang.org/stable/core/mem/fn.discriminant.html#accessing-the-numeric-value-of-the-discriminant>
    pub fn discriminant(&self) -> u16 {
        unsafe { *<*const Self>::from(self).cast::<u16>() }
    }

    // pub fn respond_with_failure(self, reason: impl Into<String>) -> Packet {
    //     Packet::GeneralFailure(packets::GeneralFailure::new(
    //         reason
    //     ))
    // }
}

pub struct Listener {
    path: std::path::PathBuf,
    receiver: tokio::sync::mpsc::Receiver<UnixStream>,
}
impl Listener {
    pub async fn new(path: impl AsRef<std::path::Path>) -> Result<Self, std::io::Error> {
        let path = path.as_ref().to_owned();
        
        // lockfile ensures there is only one legit host at a time
        match tokio::fs::remove_file(&path).await {
            Ok(()) => tracing::debug!(?path, "removed stale ipc socket file"),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }

        let listener = UnixListener::bind(&path)?;
        let (tx, rx) = tokio::sync::mpsc::channel(2);

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        if tx.send(stream).await.is_err() {
                            break; // channel closed
                        }
                    }
                    Err(e) => {
                        eprintln!("IPC accept error: {e}");
                        break;
                    }
                }
            }
        });

        Ok(Self {
            path,
            receiver: rx,
        })
    }

    async fn next_connection(&mut self) -> Option<PacketConnection> {
        self.receiver.recv().await.map(PacketConnection::from_stream)
    }
}
impl Drop for Listener {
    fn drop(&mut self) {
        match std::fs::remove_file(&self.path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => tracing::debug!(?self.path, "ipc socket file already gone"),
            Err(err) => tracing::warn!(%err, ?self.path, "could not remove ipc socket file"), // best-effort attempt
        }
    }
}


#[derive(Debug)]
pub struct PacketConnection {
    outgoing: Framed<
        FramedWrite<
            OwnedWriteHalf,
            LengthDelimitedCodec
        >,
        Packet,
        Packet,
        SymmetricalBincode<Packet>
    >,

    incoming: Framed<
        FramedRead<
            OwnedReadHalf,
            LengthDelimitedCodec
        >,
        Packet,
        Packet,
        SymmetricalBincode<Packet>
    >
}
impl PacketConnection {
    pub async fn from_path(path: impl AsRef<std::path::Path>) -> Result<Self, std::io::Error> {
        Ok(Self::from_stream(tokio::net::UnixStream::connect(path).await?))
    }

    pub fn from_stream(stream: UnixStream) -> Self {
        let (read, write) = stream.into_split();

        let framed_write = FramedWrite::new(write, LengthDelimitedCodec::new());
        let framed_read = FramedRead::new(read, LengthDelimitedCodec::new());

        let outgoing = SymmetricallyFramed::new(framed_write, SymmetricalBincode::<Packet>::default());
        let incoming = SymmetricallyFramed::new(framed_read, SymmetricalBincode::<Packet>::default());

        Self { outgoing, incoming }
    }

    pub async fn recv(&mut self) -> Result<Option<Packet>, std::io::Error> {
        self.incoming.next().await.transpose()
    }

    pub async fn send(&mut self, packet: impl Into<Packet>) -> Result<(), std::io::Error> {
        self.outgoing.send(packet.into()).await?;
        Ok(())
    }
}

pub async fn listen(
    context: Arc<Mutex<crate::PollingContext>>,
    config: Arc<Mutex<crate::config::Config>>
) -> tokio::task::AbortHandle {
    let socket_path = { config.lock().await.socket_path.clone() };
    let mut listener = Listener::new(socket_path).await.expect("failed to listen on IPC socket");

    tokio::spawn(async move {
        loop {
            let Some(mut connection) = listener.next_connection().await else { break };

            let hello = match connection.recv().await {
                Ok(None) => return,
                Ok(Some(Packet::Hello(hello))) => hello,
                Ok(Some(got)) => {
                    tracing::error!("IPC wanted hello first but got one with {}, closing connection", got.discriminant());
                    return;
                }
                Err(err) => {
                    tracing::error!(?err, "IPC recv error");
                    return;
                }
            };

            #[allow(clippy::while_let_loop)]
            loop {
                match act_upon_next_packet(&hello, &mut connection, context.clone(), config.clone()).await {
                    ConnectionAction::Continue => {},
                    ConnectionAction::Break => break,
                }
            }
        }
    }).abort_handle()
}

enum ConnectionAction {
    Continue,
    Break,
}

#[expect(clippy::significant_drop_tightening, reason = "holding a config lock is desired, since possible race conditions would be wacky")]
async fn act_upon_next_packet(
    hello: &packets::Hello,
    connection: &mut PacketConnection,
    context: Arc<Mutex<crate::PollingContext>>,
    config: Arc<Mutex<crate::config::Config>>
) -> ConnectionAction {
    match connection.recv().await {
        Ok(Some(packet)) => match packet {
            Packet::Hello(hello) => {
                tracing::error!(?hello, "received duplicate hello; closing connection");
                ConnectionAction::Break
            },
            Packet::GeneralFailure(failure) => {
                tracing::error!(?failure, "received failure from process {pid}", pid = hello.process);
                ConnectionAction::Continue
            }
            Packet::ReloadConfiguration => {
                use crate::config::LoadableConfig;
                let mut config = config.lock().await;
                if let Err(err) = config.reload_from_disk().await {
                    tracing::error!(?err, "could not update config");
                    return ConnectionAction::Continue;
                }
                context.lock().await.reload_from_config(&config).await;
                ConnectionAction::Continue
            }
        },
        Ok(None) => ConnectionAction::Break,
        Err(err) => {
            tracing::error!(?err, "IPC recv error");
            ConnectionAction::Break
        }
    }
}
